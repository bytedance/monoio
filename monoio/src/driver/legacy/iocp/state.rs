use core::fmt::Debug;
use std::{
    marker::PhantomPinned,
    os::windows::prelude::RawSocket,
    pin::Pin,
    sync::{Arc, Mutex},
};

use windows_sys::Win32::{
    Foundation::{ERROR_INVALID_HANDLE, ERROR_IO_PENDING, HANDLE, STATUS_CANCELLED},
    Networking::WinSock::{
        WSAGetLastError, WSAIoctl, SIO_BASE_HANDLE, SIO_BSP_HANDLE, SIO_BSP_HANDLE_POLL,
        SIO_BSP_HANDLE_SELECT, SOCKET_ERROR,
    },
    System::WindowsProgramming::IO_STATUS_BLOCK,
};

use super::{afd, from_overlapped, into_overlapped, Afd, AfdPollInfo, Event};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SockPollStatus {
    Idle,
    Pending,
    Cancelled,
}

#[derive(Debug)]
pub struct SocketStateInner {
    pub inner: Option<Pin<Arc<Mutex<SockState>>>>,
    pub token: mio::Token,
    pub interest: mio::Interest,
}

#[derive(Debug)]
pub struct SocketState {
    pub socket: RawSocket,
    pub inner: Arc<Mutex<SocketStateInner>>,
}

impl SocketState {
    pub fn new(socket: RawSocket) -> Self {
        Self {
            socket,
            inner: Arc::new(Mutex::new(SocketStateInner {
                inner: None,
                token: mio::Token(0),
                interest: mio::Interest::READABLE,
            })),
        }
    }
}

pub struct SockState {
    pub iosb: IO_STATUS_BLOCK,
    pub poll_info: AfdPollInfo,
    pub afd: Arc<Afd>,

    pub base_socket: RawSocket,

    pub user_evts: u32,
    pub pending_evts: u32,

    pub user_data: u64,

    pub poll_status: SockPollStatus,
    pub delete_pending: bool,

    pub error: Option<i32>,

    _pinned: PhantomPinned,
}

impl SockState {
    pub fn new(raw_socket: RawSocket, afd: Arc<Afd>) -> std::io::Result<SockState> {
        Ok(SockState {
            iosb: unsafe { std::mem::zeroed() },
            poll_info: unsafe { std::mem::zeroed() },
            afd,
            base_socket: get_base_socket(raw_socket)?,
            user_evts: 0,
            pending_evts: 0,
            user_data: 0,
            poll_status: SockPollStatus::Idle,
            delete_pending: false,
            error: None,
            _pinned: PhantomPinned,
        })
    }

    pub fn update(&mut self, self_arc: &Pin<Arc<Mutex<SockState>>>) -> std::io::Result<()> {
        assert!(!self.delete_pending);

        // make sure to reset previous error before a new update
        self.error = None;

        if let SockPollStatus::Pending = self.poll_status {
            if (self.user_evts & afd::KNOWN_EVENTS & !self.pending_evts) == 0 {
                // All the events the user is interested in are already being monitored by
                // the pending poll operation. It might spuriously complete because of an
                // event that we're no longer interested in; when that happens we'll submit
                // a new poll operation with the updated event mask.
            } else {
                // A poll operation is already pending, but it's not monitoring for all the
                // events that the user is interested in. Therefore, cancel the pending
                // poll operation; when we receive it's completion package, a new poll
                // operation will be submitted with the correct event mask.
                if let Err(e) = self.cancel() {
                    self.error = e.raw_os_error();
                    return Err(e);
                }
                return Ok(());
            }
        } else if let SockPollStatus::Cancelled = self.poll_status {
            // The poll operation has already been cancelled, we're still waiting for
            // it to return. For now, there's nothing that needs to be done.
        } else if let SockPollStatus::Idle = self.poll_status {
            // No poll operation is pending; start one.
            self.poll_info.exclusive = 0;
            self.poll_info.number_of_handles = 1;
            self.poll_info.timeout = i64::MAX;
            self.poll_info.handles[0].handle = self.base_socket as HANDLE;
            self.poll_info.handles[0].status = 0;
            self.poll_info.handles[0].events = self.user_evts | afd::POLL_LOCAL_CLOSE;

            // Increase the ref count as the memory will be used by the kernel.
            let overlapped_ptr = into_overlapped(self_arc.clone());

            let result = unsafe {
                self.afd
                    .poll(&mut self.poll_info, &mut self.iosb, overlapped_ptr)
            };
            if let Err(e) = result {
                let code = e.raw_os_error().unwrap();
                if code == ERROR_IO_PENDING as i32 {
                    // Overlapped poll operation in progress; this is expected.
                } else {
                    // Since the operation failed it means the kernel won't be
                    // using the memory any more.
                    drop(from_overlapped(overlapped_ptr as *mut _));
                    if code == ERROR_INVALID_HANDLE as i32 {
                        // Socket closed; it'll be dropped.
                        self.mark_delete();
                        return Ok(());
                    } else {
                        self.error = e.raw_os_error();
                        return Err(e);
                    }
                }
            }

            self.poll_status = SockPollStatus::Pending;
            self.pending_evts = self.user_evts;
        } else {
            unreachable!("Invalid poll status during update")
        }

        Ok(())
    }

    pub fn feed_event(&mut self) -> Option<Event> {
        self.poll_status = SockPollStatus::Idle;
        self.pending_evts = 0;

        let mut afd_events = 0;
        // We use the status info in IO_STATUS_BLOCK to determine the socket poll status. It is
        // unsafe to use a pointer of IO_STATUS_BLOCK.
        unsafe {
            if self.delete_pending {
                return None;
            } else if self.iosb.Anonymous.Status == STATUS_CANCELLED {
                // The poll request was cancelled by CancelIoEx.
            } else if self.iosb.Anonymous.Status < 0 {
                // The overlapped request itself failed in an unexpected way.
                afd_events = afd::POLL_CONNECT_FAIL;
            } else if self.poll_info.number_of_handles < 1 {
                // This poll operation succeeded but didn't report any socket events.
            } else if self.poll_info.handles[0].events & afd::POLL_LOCAL_CLOSE != 0 {
                // The poll operation reported that the socket was closed.
                self.mark_delete();
                return None;
            } else {
                afd_events = self.poll_info.handles[0].events;
            }
        }

        afd_events &= self.user_evts;

        if afd_events == 0 {
            return None;
        }

        self.user_evts &= !afd_events;

        Some(Event {
            data: self.user_data,
            flags: afd_events,
        })
    }

    pub fn mark_delete(&mut self) {
        if !self.delete_pending {
            if let SockPollStatus::Pending = self.poll_status {
                drop(self.cancel());
            }

            self.delete_pending = true;
        }
    }

    pub fn set_event(&mut self, ev: Event) -> bool {
        // afd::POLL_CONNECT_FAIL and afd::POLL_ABORT are always reported, even when not requested
        // by the caller.
        let events = ev.flags | afd::POLL_CONNECT_FAIL | afd::POLL_ABORT;

        self.user_evts = events;
        self.user_data = ev.data;

        (events & !self.pending_evts) != 0
    }

    pub fn cancel(&mut self) -> std::io::Result<()> {
        match self.poll_status {
            SockPollStatus::Pending => {}
            _ => unreachable!("Invalid poll status during cancel"),
        };
        unsafe {
            self.afd.cancel(&mut self.iosb)?;
        }
        self.poll_status = SockPollStatus::Cancelled;
        self.pending_evts = 0;
        Ok(())
    }
}

impl Debug for SockState {
    #[allow(unused_variables)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unimplemented!()
    }
}

impl Drop for SockState {
    fn drop(&mut self) {
        self.mark_delete();
    }
}

fn get_base_socket(raw_socket: RawSocket) -> std::io::Result<RawSocket> {
    let res = try_get_base_socket(raw_socket, SIO_BASE_HANDLE);
    if let Ok(base_socket) = res {
        return Ok(base_socket);
    }

    // The `SIO_BASE_HANDLE` should not be intercepted by LSPs, therefore
    // it should not fail as long as `raw_socket` is a valid socket. See
    // https://docs.microsoft.com/en-us/windows/win32/winsock/winsock-ioctls.
    // However, at least one known LSP deliberately breaks it, so we try
    // some alternative IOCTLs, starting with the most appropriate one.
    for &ioctl in &[SIO_BSP_HANDLE_SELECT, SIO_BSP_HANDLE_POLL, SIO_BSP_HANDLE] {
        if let Ok(base_socket) = try_get_base_socket(raw_socket, ioctl) {
            // Since we know now that we're dealing with an LSP (otherwise
            // SIO_BASE_HANDLE would't have failed), only return any result
            // when it is different from the original `raw_socket`.
            if base_socket != raw_socket {
                return Ok(base_socket);
            }
        }
    }

    // If the alternative IOCTLs also failed, return the original error.
    let os_error = res.unwrap_err();
    let err = std::io::Error::from_raw_os_error(os_error);
    Err(err)
}

fn try_get_base_socket(raw_socket: RawSocket, ioctl: u32) -> Result<RawSocket, i32> {
    let mut base_socket: RawSocket = 0;
    let mut bytes: u32 = 0;
    let result = unsafe {
        WSAIoctl(
            raw_socket as usize,
            ioctl,
            std::ptr::null_mut(),
            0,
            &mut base_socket as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<RawSocket>() as u32,
            &mut bytes,
            std::ptr::null_mut(),
            None,
        )
    };

    if result != SOCKET_ERROR {
        Ok(base_socket)
    } else {
        Err(unsafe { WSAGetLastError() })
    }
}
