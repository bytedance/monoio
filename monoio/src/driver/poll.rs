use std::{io, task::Context, time::Duration};

use super::{ready::Direction, scheduled_io::ScheduledIo};
use crate::{driver::op::CompletionMeta, utils::slab::Slab};

/// Poller with io dispatch.
// TODO: replace legacy impl with this Poll.
pub(crate) struct Poll {
    pub(crate) io_dispatch: Slab<ScheduledIo>,
    poll: mio::Poll,
    events: mio::Events,
}

impl Poll {
    #[inline]
    pub(crate) fn with_capacity(capacity: usize) -> io::Result<Self> {
        Ok(Self {
            io_dispatch: Slab::new(),
            poll: mio::Poll::new()?,
            events: mio::Events::with_capacity(capacity),
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
                let ready = super::ready::Ready::from(event);
                ref_mut.set_readiness(|curr| curr | ready);
                ref_mut.wake(ready);
            }
        }
        Ok(())
    }

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
}

#[cfg(unix)]
impl std::os::fd::AsRawFd for Poll {
    #[inline]
    fn as_raw_fd(&self) -> std::os::fd::RawFd {
        self.poll.as_raw_fd()
    }
}
