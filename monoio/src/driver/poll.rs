use std::{io, task::Context, time::Duration};

#[cfg(windows)]
use super::legacy::iocp;
use super::{
    ready::{Direction, Ready},
    scheduled_io::ScheduledIo,
};
#[cfg(feature = "poll-io")]
use crate::driver::op::CompletionMeta;
use crate::utils::slab::Slab;

/// Poller with io dispatch.
// TODO: replace legacy impl with this Poll.
pub(crate) struct Poll {
    pub(crate) io_dispatch: Slab<ScheduledIo>,
    #[cfg(unix)]
    pub(crate) events: mio::Events,
    #[cfg(unix)]
    pub(crate) poll: mio::Poll,
    #[cfg(windows)]
    pub(crate) events: iocp::Events,
    #[cfg(windows)]
    pub(crate) poll: iocp::Poller,
}

impl Poll {
    #[inline]
    pub(crate) fn with_capacity(capacity: usize) -> io::Result<Self> {
        Ok(Self {
            io_dispatch: Slab::new(),
            #[cfg(unix)]
            events: mio::Events::with_capacity(capacity),
            #[cfg(windows)]
            events: iocp::Events::with_capacity(capacity),
            #[cfg(unix)]
            poll: mio::Poll::new()?,
            #[cfg(windows)]
            poll: iocp::Poller::new()?,
        })
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn clear_readiness(io_dispatch: &mut Slab<ScheduledIo>, token: usize, clear: Ready) {
        let mut sio = match io_dispatch.get(token) {
            Some(io) => io,
            None => {
                return;
            }
        };
        let ref_mut = sio.as_mut();
        ref_mut.set_readiness(|curr| curr - clear);
    }

    #[inline]
    pub(crate) fn dispatch(io_dispatch: &mut Slab<ScheduledIo>, token: usize, ready: Ready) {
        let mut sio = match io_dispatch.get(token) {
            Some(io) => io,
            None => {
                return;
            }
        };
        let ref_mut = sio.as_mut();
        ref_mut.set_readiness(|curr| curr | ready);
        ref_mut.wake(ready);
    }

    #[inline]
    pub(crate) fn poll_inside(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.poll.poll(&mut self.events, timeout)
    }

    #[cfg(all(feature = "poll-io", target_os = "linux"))]
    #[inline]
    pub(crate) fn tick(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        match self.poll.poll(&mut self.events, timeout) {
            Ok(_) => {}
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
        for event in self.events.iter() {
            let token = event.token();

            if let Some(mut sio) = self.io_dispatch.get(token.0) {
                let ref_mut = sio.as_mut();
                let ready = super::ready::Ready::from_mio(event);
                ref_mut.set_readiness(|curr| curr | ready);
                ref_mut.wake(ready);
            }
        }
        Ok(())
    }

    #[cfg(windows)]
    pub(crate) fn register(
        &mut self,
        state: &mut iocp::SocketState,
        interest: mio::Interest,
    ) -> io::Result<usize> {
        let io = ScheduledIo::default();
        let token = self.io_dispatch.insert(io);

        match self.poll.register(state, mio::Token(token), interest) {
            Ok(_) => Ok(token),
            Err(e) => {
                self.io_dispatch.remove(token);
                Err(e)
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn deregister(
        &mut self,
        token: usize,
        state: &mut iocp::SocketState,
    ) -> io::Result<()> {
        // try to deregister fd first, on success we will remove it from slab.
        match self.poll.deregister(state) {
            Ok(_) => {
                self.io_dispatch.remove(token);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[cfg(unix)]
    pub(crate) fn register(
        &mut self,
        source: &mut impl mio::event::Source,
        interest: mio::Interest,
    ) -> io::Result<usize> {
        let token = self.io_dispatch.insert(ScheduledIo::new());
        let registry = self.poll.registry();
        match registry.register(source, mio::Token(token), interest) {
            Ok(_) => Ok(token),
            Err(e) => {
                self.io_dispatch.remove(token);
                Err(e)
            }
        }
    }

    #[cfg(unix)]
    pub(crate) fn deregister(
        &mut self,
        source: &mut impl mio::event::Source,
        token: usize,
    ) -> io::Result<()> {
        match self.poll.registry().deregister(source) {
            Ok(_) => {
                self.io_dispatch.remove(token);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[cfg(feature = "poll-io")]
    #[inline]
    pub(crate) fn poll_syscall(
        &mut self,
        cx: &mut Context<'_>,
        token: usize,
        direction: Direction,
        syscall: impl FnOnce() -> io::Result<u32>,
    ) -> std::task::Poll<CompletionMeta> {
        let mut scheduled_io = self.io_dispatch.get(token).expect("scheduled_io lost");
        let ref_mut = scheduled_io.as_mut();
        ready!(ref_mut.poll_readiness(cx, direction));
        match syscall() {
            Ok(n) => std::task::Poll::Ready(CompletionMeta {
                result: Ok(n),
                flags: 0,
            }),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                ref_mut.clear_readiness(direction.mask());
                ref_mut.set_waker(cx, direction);
                std::task::Poll::Pending
            }
            Err(e) => std::task::Poll::Ready(CompletionMeta {
                result: Err(e),
                flags: 0,
            }),
        }
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn poll_readiness(
        &mut self,
        cx: &mut Context<'_>,
        token: usize,
        direction: Direction,
    ) -> std::task::Poll<Ready> {
        let mut scheduled_io = self.io_dispatch.get(token).expect("scheduled_io lost");
        let ref_mut = scheduled_io.as_mut();
        ref_mut.poll_readiness(cx, direction)
    }
}

#[cfg(unix)]
impl std::os::fd::AsRawFd for Poll {
    #[inline]
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.poll.as_raw_fd()
    }
}
