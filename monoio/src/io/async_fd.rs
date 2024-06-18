use std::{
    io,
    os::fd::AsRawFd,
    task::{Context, Poll},
};

pub use crate::driver::ready::Ready;
use crate::driver::{poll::Poll as LegacyPoll, ready::Direction, shared_fd::SharedFd, CURRENT};

/// Associates an IO object backed by a Unix file descriptor with the async runtime.
pub struct AsyncFd<T: AsRawFd> {
    _fd: SharedFd,
    token: usize,
    inner: Option<T>,
}

/// Represents an IO-ready event detected on a particular file descriptor that
/// has not yet been acknowledged. This is a `must_use` structure to help ensure
/// that you do not forget to explicitly clear (or not clear) the event.
#[must_use = "You must explicitly choose whether to clear the readiness state by calling a method \
              on ReadyGuard"]
pub struct AsyncFdReadyGuard<'a, T: AsRawFd> {
    async_fd: &'a AsyncFd<T>,
    event: Ready,
    token: usize,
}

/// Represents an IO-ready event detected on a particular file descriptor that
/// has not yet been acknowledged. This is a `must_use` structure to help ensure
/// that you do not forget to explicitly clear (or not clear) the event.
#[must_use = "You must explicitly choose whether to clear the readiness state by calling a method \
              on ReadyGuard"]
pub struct AsyncFdReadyMutGuard<'a, T: AsRawFd> {
    async_fd: &'a mut AsyncFd<T>,
    event: Ready,
    token: usize,
}

impl<T: AsRawFd> AsyncFd<T> {
    /// Creates an [`AsyncFd`] backed by (and taking ownership of) an object
    /// implementing [`AsRawFd`]. The backing file descriptor is cached at the
    /// time of creation.
    #[inline]
    pub fn new(inner: T) -> io::Result<Self> {
        let fd = SharedFd::new::<true>(inner.as_raw_fd())?;
        let token = fd.registered_index().expect("registered index must exist");
        fd.set_close_on_drop(false);
        Ok(Self {
            _fd: fd,
            token,
            inner: Some(inner),
        })
    }

    /// Returns a shared reference to the backing object of this [`AsyncFd`].
    #[inline]
    pub fn get_ref(&self) -> &T {
        self.inner.as_ref().unwrap()
    }

    /// Returns a mutable reference to the backing object of this [`AsyncFd`].
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.as_mut().unwrap()
    }

    /// Deregisters this file descriptor and returns ownership of the backing
    /// object.
    pub fn into_inner(mut self) -> T {
        self.inner.take().unwrap()
    }

    /// Polls for read readiness.
    #[inline]
    pub fn poll_read_ready<'a>(
        &'a self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<AsyncFdReadyGuard<'a, T>>> {
        self.poll_ready_with_direction(cx, Direction::Read)
    }

    /// Polls for read readiness.
    #[inline]
    pub fn poll_read_ready_mut<'a>(
        &'a mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<AsyncFdReadyMutGuard<'a, T>>> {
        self.poll_ready_with_direction_mut(cx, Direction::Read)
    }

    /// Polls for write readiness.
    #[inline]
    pub fn poll_write_ready<'a>(
        &'a self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<AsyncFdReadyGuard<'a, T>>> {
        self.poll_ready_with_direction(cx, Direction::Write)
    }

    /// Polls for write readiness.
    #[inline]
    pub fn poll_write_ready_mut<'a>(
        &'a mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<AsyncFdReadyMutGuard<'a, T>>> {
        self.poll_ready_with_direction_mut(cx, Direction::Write)
    }

    #[inline]
    fn poll_ready_with_direction<'a>(
        &'a self,
        cx: &mut Context<'_>,
        direction: Direction,
    ) -> Poll<io::Result<AsyncFdReadyGuard<'a, T>>> {
        let r = CURRENT.with(|c| match c {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            crate::driver::Inner::Uring(inner) => unsafe {
                (*inner.get())
                    .poller
                    .poll_readiness(cx, self.token, direction)
            },
            #[cfg(feature = "legacy")]
            crate::driver::Inner::Legacy(inner) => unsafe {
                (*inner.get())
                    .poller
                    .poll_readiness(cx, self.token, direction)
            },
        });
        let event = ready!(r);

        Poll::Ready(Ok(AsyncFdReadyGuard {
            async_fd: self,
            event,
            token: self.token,
        }))
    }

    #[inline]
    fn poll_ready_with_direction_mut<'a>(
        &'a mut self,
        cx: &mut Context<'_>,
        direction: Direction,
    ) -> Poll<io::Result<AsyncFdReadyMutGuard<'a, T>>> {
        let token = self.token;
        let r = CURRENT.with(|c| match c {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            crate::driver::Inner::Uring(inner) => unsafe {
                (*inner.get()).poller.poll_readiness(cx, token, direction)
            },
            #[cfg(feature = "legacy")]
            crate::driver::Inner::Legacy(inner) => unsafe {
                (*inner.get()).poller.poll_readiness(cx, token, direction)
            },
        });
        let event = ready!(r);

        Poll::Ready(Ok(AsyncFdReadyMutGuard {
            async_fd: self,
            event,
            token,
        }))
    }
}

impl<'a, T: AsRawFd> AsyncFdReadyGuard<'a, T> {
    /// Indicates to the rumtime that the file descriptor is no longer ready.
    /// All internal readiness flags will be cleared.
    pub fn clear_ready(&mut self) {
        CURRENT.with(|c| match c {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            crate::driver::Inner::Uring(inner) => unsafe {
                LegacyPoll::clear_readiness(
                    &mut (*inner.get()).poller.io_dispatch,
                    self.token,
                    self.event,
                )
            },
            #[cfg(feature = "legacy")]
            crate::driver::Inner::Legacy(inner) => unsafe {
                LegacyPoll::clear_readiness(
                    &mut (*inner.get()).poller.io_dispatch,
                    self.token,
                    self.event,
                )
            },
        });
    }

    /// Indicates to the rumtime that the file descriptor is no longer ready.
    /// The internal readiness flag will be cleared
    pub fn clear_ready_matching(&mut self, clear: Ready) {
        CURRENT.with(|c| match c {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            crate::driver::Inner::Uring(inner) => unsafe {
                LegacyPoll::clear_readiness(
                    &mut (*inner.get()).poller.io_dispatch,
                    self.token,
                    clear,
                )
            },
            #[cfg(feature = "legacy")]
            crate::driver::Inner::Legacy(inner) => unsafe {
                LegacyPoll::clear_readiness(
                    &mut (*inner.get()).poller.io_dispatch,
                    self.token,
                    clear,
                )
            },
        });
    }

    /// Performs the provided IO operation.
    pub fn try_io<R>(
        &mut self,
        f: impl FnOnce(&'a AsyncFd<T>) -> io::Result<R>,
    ) -> Result<io::Result<R>, TryIoError> {
        let result = f(self.async_fd);

        match result {
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                self.clear_ready();
                Err(TryIoError(()))
            }
            result => Ok(result),
        }
    }
}

impl<'a, T: AsRawFd> AsyncFdReadyMutGuard<'a, T> {
    /// Indicates to the rumtime that the file descriptor is no longer ready.
    /// All internal readiness flags will be cleared.
    pub fn clear_ready(&mut self) {
        CURRENT.with(|c| match c {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            crate::driver::Inner::Uring(inner) => unsafe {
                LegacyPoll::clear_readiness(
                    &mut (*inner.get()).poller.io_dispatch,
                    self.token,
                    self.event,
                )
            },
            #[cfg(feature = "legacy")]
            crate::driver::Inner::Legacy(inner) => unsafe {
                LegacyPoll::clear_readiness(
                    &mut (*inner.get()).poller.io_dispatch,
                    self.token,
                    self.event,
                )
            },
        });
    }

    /// Indicates to the rumtime that the file descriptor is no longer ready.
    /// The internal readiness flag will be cleared
    pub fn clear_ready_matching(&mut self, clear: Ready) {
        CURRENT.with(|c| match c {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            crate::driver::Inner::Uring(inner) => unsafe {
                LegacyPoll::clear_readiness(
                    &mut (*inner.get()).poller.io_dispatch,
                    self.token,
                    clear,
                )
            },
            #[cfg(feature = "legacy")]
            crate::driver::Inner::Legacy(inner) => unsafe {
                LegacyPoll::clear_readiness(
                    &mut (*inner.get()).poller.io_dispatch,
                    self.token,
                    clear,
                )
            },
        });
    }

    /// Performs the provided IO operation.
    pub fn try_io<R>(
        &mut self,
        f: impl FnOnce(&mut AsyncFd<T>) -> io::Result<R>,
    ) -> Result<io::Result<R>, TryIoError> {
        let result = f(self.async_fd);

        match result {
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                self.clear_ready();
                Err(TryIoError(()))
            }
            result => Ok(result),
        }
    }
}

/// The error type returned by [`try_io`].
///
/// This error indicates that the IO resource returned a [`WouldBlock`] error.
#[derive(Debug)]
pub struct TryIoError(());
