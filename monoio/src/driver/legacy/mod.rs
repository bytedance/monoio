//! Monoio Legacy Driver.

use std::{
    cell::UnsafeCell,
    io,
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};

use self::{ready::Ready, scheduled_io::ScheduledIo};
use super::{
    op::{CompletionMeta, Op, OpAble},
    Driver, Inner, CURRENT,
};
use crate::utils::slab::Slab;

#[cfg(windows)]
pub(super) mod iocp;
pub(crate) mod ready;
mod scheduled_io;

#[cfg(feature = "sync")]
mod waker;
#[cfg(feature = "sync")]
pub(crate) use waker::UnparkHandle;

pub(crate) struct LegacyInner {
    pub(crate) io_dispatch: Slab<ScheduledIo>,
    #[cfg(unix)]
    events: Option<mio::Events>,
    #[cfg(unix)]
    poll: mio::Poll,
    #[cfg(windows)]
    events: Option<iocp::Events>,
    #[cfg(windows)]
    poll: iocp::Poller,

    #[cfg(feature = "sync")]
    shared_waker: std::sync::Arc<waker::EventWaker>,

    // Waker receiver
    #[cfg(feature = "sync")]
    waker_receiver: flume::Receiver<std::task::Waker>,
}

/// Driver with Poll-like syscall.
pub struct LegacyDriver {
    inner: Rc<UnsafeCell<LegacyInner>>,

    // Used for drop
    #[cfg(feature = "sync")]
    thread_id: usize,
}

#[cfg(feature = "sync")]
const TOKEN_WAKEUP: mio::Token = mio::Token(1 << 31);

impl LegacyDriver {
    const DEFAULT_ENTRIES: u32 = 1024;

    pub(crate) fn new() -> io::Result<Self> {
        Self::new_with_entries(Self::DEFAULT_ENTRIES)
    }

    pub(crate) fn new_with_entries(entries: u32) -> io::Result<Self> {
        #[cfg(unix)]
        let poll = mio::Poll::new()?;
        #[cfg(windows)]
        let poll = iocp::Poller::new()?;

        #[cfg(all(unix, feature = "sync"))]
        let shared_waker = std::sync::Arc::new(waker::EventWaker::new(mio::Waker::new(
            poll.registry(),
            TOKEN_WAKEUP,
        )?));
        #[cfg(all(windows, feature = "sync"))]
        let shared_waker = std::sync::Arc::new(waker::EventWaker::new(iocp::Waker::new(
            &poll,
            TOKEN_WAKEUP,
        )?));
        #[cfg(feature = "sync")]
        let (waker_sender, waker_receiver) = flume::unbounded::<std::task::Waker>();
        #[cfg(feature = "sync")]
        let thread_id = crate::builder::BUILD_THREAD_ID.with(|id| *id);

        let inner = LegacyInner {
            io_dispatch: Slab::new(),
            #[cfg(unix)]
            events: Some(mio::Events::with_capacity(entries as usize)),
            #[cfg(unix)]
            poll,
            #[cfg(windows)]
            events: Some(iocp::Events::with_capacity(entries as usize)),
            #[cfg(windows)]
            poll,
            #[cfg(feature = "sync")]
            shared_waker,
            #[cfg(feature = "sync")]
            waker_receiver,
        };
        let driver = Self {
            inner: Rc::new(UnsafeCell::new(inner)),
            #[cfg(feature = "sync")]
            thread_id,
        };

        // Register unpark handle
        #[cfg(feature = "sync")]
        {
            let unpark = driver.unpark();
            super::thread::register_unpark_handle(thread_id, unpark.into());
            super::thread::register_waker_sender(thread_id, waker_sender);
        }

        Ok(driver)
    }

    fn inner_park(&self, mut timeout: Option<Duration>) -> io::Result<()> {
        let inner = unsafe { &mut *self.inner.get() };

        #[allow(unused_mut)]
        let mut need_wait = true;
        #[cfg(feature = "sync")]
        {
            // Process foreign wakers
            while let Ok(w) = inner.waker_receiver.try_recv() {
                w.wake();
                need_wait = false;
            }

            // Set status as not awake if we are going to sleep
            if need_wait {
                inner
                    .shared_waker
                    .awake
                    .store(false, std::sync::atomic::Ordering::Release);
            }

            // Process foreign wakers left
            while let Ok(w) = inner.waker_receiver.try_recv() {
                w.wake();
                need_wait = false;
            }
        }

        if !need_wait {
            timeout = Some(Duration::ZERO);
        }

        // here we borrow 2 mut self, but its safe.
        let events = unsafe { (*self.inner.get()).events.as_mut().unwrap_unchecked() };
        match inner.poll.poll(events, timeout) {
            Ok(_) => {}
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
        for event in events.iter() {
            let token = event.token();

            #[cfg(feature = "sync")]
            if token != TOKEN_WAKEUP {
                inner.dispatch(token, Ready::from_mio(event));
            }

            #[cfg(not(feature = "sync"))]
            inner.dispatch(token, Ready::from_mio(event));
        }
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn register(
        this: &Rc<UnsafeCell<LegacyInner>>,
        state: &mut iocp::SockState,
        interest: mio::Interest,
    ) -> io::Result<usize> {
        let inner = unsafe { &mut *this.get() };
        let io = ScheduledIo::default();
        let token = inner.io_dispatch.insert(io);

        match inner.poll.register(state, mio::Token(token), interest) {
            Ok(_) => Ok(token),
            Err(e) => {
                inner.io_dispatch.remove(token);
                Err(e)
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn deregister(
        this: &Rc<UnsafeCell<LegacyInner>>,
        token: usize,
        state: &mut iocp::SockState,
    ) -> io::Result<()> {
        let inner = unsafe { &mut *this.get() };

        // try to deregister fd first, on success we will remove it from slab.
        match inner.poll.deregister(state) {
            Ok(_) => {
                inner.io_dispatch.remove(token);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[cfg(unix)]
    pub(crate) fn register(
        this: &Rc<UnsafeCell<LegacyInner>>,
        source: &mut impl mio::event::Source,
        interest: mio::Interest,
    ) -> io::Result<usize> {
        let inner = unsafe { &mut *this.get() };
        let io = ScheduledIo::default();
        let token = inner.io_dispatch.insert(io);

        let registry = inner.poll.registry();
        match registry.register(source, mio::Token(token), interest) {
            Ok(_) => Ok(token),
            Err(e) => {
                inner.io_dispatch.remove(token);
                Err(e)
            }
        }
    }

    #[cfg(unix)]
    pub(crate) fn deregister(
        this: &Rc<UnsafeCell<LegacyInner>>,
        token: usize,
        source: &mut impl mio::event::Source,
    ) -> io::Result<()> {
        let inner = unsafe { &mut *this.get() };

        // try to deregister fd first, on success we will remove it from slab.
        match inner.poll.registry().deregister(source) {
            Ok(_) => {
                inner.io_dispatch.remove(token);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

impl LegacyInner {
    fn dispatch(&mut self, token: mio::Token, ready: Ready) {
        let mut sio = match self.io_dispatch.get(token.0) {
            Some(io) => io,
            None => {
                return;
            }
        };
        let ref_mut = sio.as_mut();
        ref_mut.set_readiness(|curr| curr | ready);
        ref_mut.wake(ready);
    }

    pub(crate) fn poll_op<T: OpAble>(
        this: &Rc<UnsafeCell<Self>>,
        data: &mut T,
        cx: &mut Context<'_>,
    ) -> Poll<CompletionMeta> {
        let inner = unsafe { &mut *this.get() };
        let (direction, index) = match data.legacy_interest() {
            Some(x) => x,
            None => {
                // if there is no index provided, it means the action does not rely on fd
                // readiness. do syscall right now.
                return Poll::Ready(CompletionMeta {
                    result: OpAble::legacy_call(data),
                    flags: 0,
                });
            }
        };

        // wait io ready and do syscall
        let mut scheduled_io = inner.io_dispatch.get(index).expect("scheduled_io lost");
        let ref_mut = scheduled_io.as_mut();
        loop {
            let readiness = ready!(ref_mut.poll_readiness(cx, direction));

            // check if canceled
            if readiness.is_canceled() {
                // clear CANCELED part only
                ref_mut.clear_readiness(readiness & Ready::CANCELED);
                return Poll::Ready(CompletionMeta {
                    result: Err(io::Error::from_raw_os_error(125)),
                    flags: 0,
                });
            }

            match OpAble::legacy_call(data) {
                Ok(n) => {
                    return Poll::Ready(CompletionMeta {
                        result: Ok(n),
                        flags: 0,
                    })
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    ref_mut.clear_readiness(direction.mask());
                    continue;
                }
                Err(e) => {
                    return Poll::Ready(CompletionMeta {
                        result: Err(e),
                        flags: 0,
                    })
                }
            }
        }
    }

    pub(crate) fn cancel_op(
        this: &Rc<UnsafeCell<LegacyInner>>,
        index: usize,
        direction: ready::Direction,
    ) {
        let inner = unsafe { &mut *this.get() };
        let ready = match direction {
            ready::Direction::Read => Ready::READ_CANCELED,
            ready::Direction::Write => Ready::WRITE_CANCELED,
        };
        inner.dispatch(mio::Token(index), ready);
    }

    pub(crate) fn submit_with_data<T>(
        this: &Rc<UnsafeCell<LegacyInner>>,
        data: T,
    ) -> io::Result<Op<T>>
    where
        T: OpAble,
    {
        Ok(Op {
            driver: Inner::Legacy(this.clone()),
            // useless for legacy
            index: 0,
            data: Some(data),
        })
    }

    #[cfg(feature = "sync")]
    pub(crate) fn unpark(this: &Rc<UnsafeCell<LegacyInner>>) -> waker::UnparkHandle {
        let inner = unsafe { &*this.get() };
        let weak = std::sync::Arc::downgrade(&inner.shared_waker);
        waker::UnparkHandle(weak)
    }
}

impl Driver for LegacyDriver {
    fn with<R>(&self, f: impl FnOnce() -> R) -> R {
        let inner = Inner::Legacy(self.inner.clone());
        CURRENT.set(&inner, f)
    }

    fn submit(&self) -> io::Result<()> {
        // wait with timeout = 0
        self.park_timeout(Duration::ZERO)
    }

    fn park(&self) -> io::Result<()> {
        self.inner_park(None)
    }

    fn park_timeout(&self, duration: Duration) -> io::Result<()> {
        self.inner_park(Some(duration))
    }

    #[cfg(feature = "sync")]
    type Unpark = waker::UnparkHandle;

    #[cfg(feature = "sync")]
    fn unpark(&self) -> Self::Unpark {
        LegacyInner::unpark(&self.inner)
    }
}

impl Drop for LegacyDriver {
    fn drop(&mut self) {
        // Deregister thread id
        #[cfg(feature = "sync")]
        {
            use crate::driver::thread::{unregister_unpark_handle, unregister_waker_sender};
            unregister_unpark_handle(self.thread_id);
            unregister_waker_sender(self.thread_id);
        }
    }
}
