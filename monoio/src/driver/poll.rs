use std::{
    io,
    ops::{Deref, DerefMut},
    task::Context,
    time::Duration,
};

#[cfg(unix)]
use mio::{event::Source, Events};
use mio::{Interest, Token};

use super::{op::MaybeFd, ready::Direction, scheduled_io::ScheduledIo};
#[cfg(windows)]
use crate::driver::iocp::{Events, Poller, SocketState};
use crate::{driver::op::CompletionMeta, utils::slab::Slab};

/// Poller with io dispatch.
pub(crate) struct Poll {
    pub(crate) io_dispatch: Slab<ScheduledIo>,
    #[cfg(unix)]
    poll: mio::Poll,
    #[cfg(unix)]
    events: Events,
    #[cfg(windows)]
    poll: Poller,
    #[cfg(windows)]
    events: Events,
}

impl Poll {
    #[inline]
    pub(crate) fn with_capacity(capacity: usize) -> io::Result<Self> {
        Ok(Self {
            io_dispatch: Slab::new(),
            #[cfg(unix)]
            poll: mio::Poll::new()?,
            #[cfg(windows)]
            poll: Poller::new()?,
            events: Events::with_capacity(capacity),
        })
    }

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

    #[cfg(unix)]
    pub(crate) fn register(
        &mut self,
        source: &mut impl Source,
        interest: Interest,
    ) -> io::Result<usize> {
        let token = self.io_dispatch.insert(ScheduledIo::new());
        let registry = self.poll.registry();
        match registry.register(source, Token(token), interest) {
            Ok(_) => Ok(token),
            Err(e) => {
                self.io_dispatch.remove(token);
                Err(e)
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn register(
        &mut self,
        source: &mut SocketState,
        interest: Interest,
    ) -> io::Result<usize> {
        let token = self.io_dispatch.insert(ScheduledIo::new());
        match self.poll.register(source, Token(token), interest) {
            Ok(_) => Ok(token),
            Err(e) => {
                self.io_dispatch.remove(token);
                Err(e)
            }
        }
    }

    #[cfg(unix)]
    pub(crate) fn deregister(&mut self, source: &mut impl Source, token: usize) -> io::Result<()> {
        match self.poll.registry().deregister(source) {
            Ok(_) => {
                self.io_dispatch.remove(token);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[cfg(windows)]
    pub(crate) fn deregister(&mut self, source: &mut SocketState, token: usize) -> io::Result<()> {
        match self.poll.deregister(source) {
            Ok(_) => {
                self.io_dispatch.remove(token);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub(crate) fn poll_syscall(
        &mut self,
        cx: &mut Context<'_>,
        token: usize,
        direction: Direction,
        syscall: impl FnOnce() -> io::Result<MaybeFd>,
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
}

#[cfg(unix)]
impl std::os::fd::AsRawFd for Poll {
    #[inline]
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.poll.as_raw_fd()
    }
}

#[cfg(unix)]
impl Deref for Poll {
    type Target = mio::Poll;

    fn deref(&self) -> &Self::Target {
        &self.poll
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsRawHandle for Poll {
    #[inline]
    fn as_raw_handle(&self) -> std::os::windows::io::RawHandle {
        self.poll.as_raw_handle()
    }
}

#[cfg(windows)]
impl Deref for Poll {
    type Target = Poller;

    fn deref(&self) -> &Self::Target {
        &self.poll
    }
}

impl DerefMut for Poll {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.poll
    }
}
