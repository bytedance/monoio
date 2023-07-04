mod afd;
mod event;
mod iocp;
mod state;
mod waker;

use std::{
    collections::VecDeque,
    os::windows::prelude::RawSocket,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

pub use afd::*;
pub use event::*;
pub use iocp::*;
pub use state::*;
pub use waker::*;
use windows_sys::Win32::{
    Foundation::WAIT_TIMEOUT,
    System::IO::{OVERLAPPED, OVERLAPPED_ENTRY},
};

pub struct Poller {
    is_polling: AtomicBool,
    cp: CompletionPort,
    update_queue: Mutex<VecDeque<Pin<Arc<Mutex<SockState>>>>>,
    afd: Mutex<Vec<Arc<Afd>>>,
}

impl Poller {
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            is_polling: AtomicBool::new(false),
            cp: CompletionPort::new(0)?,
            update_queue: Mutex::new(VecDeque::new()),
            afd: Mutex::new(Vec::new()),
        })
    }

    pub fn poll(&self, events: &mut Events, timeout: Option<Duration>) -> std::io::Result<()> {
        events.clear();

        if timeout.is_none() {
            loop {
                let len = self.poll_inner(&mut events.statuses, &mut events.events, None)?;
                if len == 0 {
                    continue;
                }
                break Ok(());
            }
        } else {
            self.poll_inner(&mut events.statuses, &mut events.events, timeout)?;
            Ok(())
        }
    }

    pub fn poll_inner(
        &self,
        entries: &mut [OVERLAPPED_ENTRY],
        events: &mut Vec<Event>,
        timeout: Option<Duration>,
    ) -> std::io::Result<usize> {
        self.is_polling.swap(true, Ordering::AcqRel);

        unsafe { self.update_sockets_events() }?;

        let result = self.cp.get_many(entries, timeout);

        self.is_polling.store(false, Ordering::Relaxed);

        match result {
            Ok(iocp_events) => Ok(unsafe { self.feed_events(events, iocp_events) }),
            Err(ref e) if e.raw_os_error() == Some(WAIT_TIMEOUT as i32) => Ok(0),
            Err(e) => Err(e),
        }
    }

    unsafe fn update_sockets_events(&self) -> std::io::Result<()> {
        let mut queue = self.update_queue.lock().unwrap();
        for sock in queue.iter_mut() {
            let mut sock_internal = sock.lock().unwrap();
            if !sock_internal.delete_pending {
                sock_internal.update(sock)?;
            }
        }

        queue.retain(|sock| sock.lock().unwrap().error.is_some());

        let mut afd = self.afd.lock().unwrap();
        afd.retain(|g| Arc::strong_count(g) > 1);
        Ok(())
    }

    unsafe fn feed_events(&self, events: &mut Vec<Event>, entries: &[OVERLAPPED_ENTRY]) -> usize {
        let mut n = 0;
        let mut update_queue = self.update_queue.lock().unwrap();
        for entry in entries.iter() {
            if entry.lpOverlapped.is_null() {
                events.push(Event::from_entry(entry));
                n += 1;
                continue;
            }

            let sock_state = from_overlapped(entry.lpOverlapped);
            let mut sock_guard = sock_state.lock().unwrap();
            if let Some(e) = sock_guard.feed_event() {
                events.push(e);
                n += 1;
            }

            if !sock_guard.delete_pending {
                update_queue.push_back(sock_state.clone());
            }
        }
        let mut afd = self.afd.lock().unwrap();
        afd.retain(|sock| Arc::strong_count(sock) > 1);
        n
    }

    pub fn register(
        &self,
        state: &mut SocketState,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        if state.inner.is_none() {
            let flags = interests_to_afd_flags(interests);

            let inner = {
                let sock = self._alloc_sock_for_rawsocket(state.socket)?;
                let event = Event {
                    flags,
                    data: token.0 as u64,
                };
                sock.lock().unwrap().set_event(event);
                sock
            };

            self.queue_state(inner.clone());
            unsafe { self.update_sockets_events_if_polling()? };
            state.inner = Some(inner);
            state.token = token;
            state.interest = interests;

            Ok(())
        } else {
            Err(std::io::ErrorKind::AlreadyExists.into())
        }
    }

    pub fn reregister(
        &self,
        state: &mut SocketState,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        if let Some(inner) = state.inner.as_mut() {
            {
                let event = Event {
                    flags: interests_to_afd_flags(interests),
                    data: token.0 as u64,
                };

                inner.lock().unwrap().set_event(event);
            }

            state.token = token;
            state.interest = interests;

            self.queue_state(inner.clone());
            unsafe { self.update_sockets_events_if_polling() }
        } else {
            Err(std::io::ErrorKind::NotFound.into())
        }
    }

    pub fn deregister(&mut self, state: &mut SocketState) -> std::io::Result<()> {
        if let Some(inner) = state.inner.as_mut() {
            {
                let mut sock_state = inner.lock().unwrap();
                sock_state.mark_delete();
            }
            state.inner = None;
            Ok(())
        } else {
            Err(std::io::ErrorKind::NotFound.into())
        }
    }

    /// This function is called by register() and reregister() to start an
    /// IOCTL_AFD_POLL operation corresponding to the registered events, but
    /// only if necessary.
    ///
    /// Since it is not possible to modify or synchronously cancel an AFD_POLL
    /// operation, and there can be only one active AFD_POLL operation per
    /// (socket, completion port) pair at any time, it is expensive to change
    /// a socket's event registration after it has been submitted to the kernel.
    ///
    /// Therefore, if no other threads are polling when interest in a socket
    /// event is (re)registered, the socket is added to the 'update queue', but
    /// the actual syscall to start the IOCTL_AFD_POLL operation is deferred
    /// until just before the GetQueuedCompletionStatusEx() syscall is made.
    ///
    /// However, when another thread is already blocked on
    /// GetQueuedCompletionStatusEx() we tell the kernel about the registered
    /// socket event(s) immediately.
    unsafe fn update_sockets_events_if_polling(&self) -> std::io::Result<()> {
        if self.is_polling.load(Ordering::Acquire) {
            self.update_sockets_events()
        } else {
            Ok(())
        }
    }

    fn queue_state(&self, sock_state: Pin<Arc<Mutex<SockState>>>) {
        let mut update_queue = self.update_queue.lock().unwrap();
        update_queue.push_back(sock_state);
    }

    fn _alloc_sock_for_rawsocket(
        &self,
        raw_socket: RawSocket,
    ) -> std::io::Result<Pin<Arc<Mutex<SockState>>>> {
        const POLL_GROUP__MAX_GROUP_SIZE: usize = 32;

        let mut afd_group = self.afd.lock().unwrap();
        if afd_group.len() == 0 {
            self._alloc_afd_group(&mut afd_group)?;
        } else {
            // + 1 reference in Vec
            if Arc::strong_count(afd_group.last().unwrap()) > POLL_GROUP__MAX_GROUP_SIZE {
                self._alloc_afd_group(&mut afd_group)?;
            }
        }
        let afd = match afd_group.last() {
            Some(arc) => arc.clone(),
            None => unreachable!("Cannot acquire afd"),
        };

        Ok(Arc::pin(Mutex::new(SockState::new(raw_socket, afd)?)))
    }

    fn _alloc_afd_group(&self, afd_group: &mut Vec<Arc<Afd>>) -> std::io::Result<()> {
        let afd = Afd::new(&self.cp)?;
        let arc = Arc::new(afd);
        afd_group.push(arc);
        Ok(())
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        loop {
            let count: usize;
            let mut statuses: [OVERLAPPED_ENTRY; 1024] = unsafe { std::mem::zeroed() };

            let result = self
                .cp
                .get_many(&mut statuses, Some(std::time::Duration::from_millis(0)));
            match result {
                Ok(events) => {
                    count = events.iter().len();
                    for event in events.iter() {
                        if event.lpOverlapped.is_null() {
                        } else {
                            // drain sock state to release memory of Arc reference
                            let _ = from_overlapped(event.lpOverlapped);
                        }
                    }
                }
                Err(_) => break,
            }

            if count == 0 {
                break;
            }
        }

        let mut afd_group = self.afd.lock().unwrap();
        afd_group.retain(|g| Arc::strong_count(g) > 1);
    }
}

pub fn from_overlapped(ptr: *mut OVERLAPPED) -> Pin<Arc<Mutex<SockState>>> {
    let sock_ptr: *const Mutex<SockState> = ptr as *const _;
    unsafe { Pin::new_unchecked(Arc::from_raw(sock_ptr)) }
}

pub fn into_overlapped(sock_state: Pin<Arc<Mutex<SockState>>>) -> *mut std::ffi::c_void {
    let overlapped_ptr: *const Mutex<SockState> =
        unsafe { Arc::into_raw(Pin::into_inner_unchecked(sock_state)) };
    overlapped_ptr as *mut _
}

pub fn interests_to_afd_flags(interests: mio::Interest) -> u32 {
    let mut flags = 0;

    if interests.is_readable() {
        flags |= READABLE_FLAGS | READ_CLOSED_FLAGS | ERROR_FLAGS;
    }

    if interests.is_writable() {
        flags |= WRITABLE_FLAGS | WRITE_CLOSED_FLAGS | ERROR_FLAGS;
    }

    flags
}
