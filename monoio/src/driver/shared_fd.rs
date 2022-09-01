#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::{cell::UnsafeCell, io, rc::Rc};

use super::CURRENT;

// Tracks in-flight operations on a file descriptor. Ensures all in-flight
// operations complete before submitting the close.
#[derive(Clone, Debug)]
pub(crate) struct SharedFd {
    inner: Rc<Inner>,
}

struct Inner {
    // Open file descriptor
    #[cfg(unix)]
    fd: RawFd,

    #[cfg(windows)]
    fd: RawHandle,

    // Waker to notify when the close operation completes.
    state: UnsafeCell<State>,
}

enum State {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    Uring(UringState),
    #[cfg(all(unix, feature = "legacy"))]
    Legacy(Option<usize>),
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner").field("fd", &self.fd).finish()
    }
}

#[cfg(all(target_os = "linux", feature = "iouring"))]
enum UringState {
    /// Initial state
    Init,

    /// Waiting for all in-flight operation to complete.
    Waiting(Option<std::task::Waker>),

    /// The FD is closing
    Closing(super::op::Op<super::op::close::Close>),

    /// The FD is fully closed
    Closed,
}

#[cfg(unix)]
impl AsRawFd for SharedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.raw_fd()
    }
}

#[cfg(windows)]
impl AsRawHandle for SharedFd {
    fn as_raw_handle(&self) -> RawHandle {
        self.raw_handle()
    }
}

impl SharedFd {
    #[cfg(unix)]
    #[allow(unreachable_code, unused)]
    pub(crate) fn new(fd: RawFd) -> io::Result<SharedFd> {
        #[cfg(all(unix, feature = "legacy"))]
        const RW_INTERESTS: mio::Interest = mio::Interest::READABLE.add(mio::Interest::WRITABLE);

        #[cfg(all(target_os = "linux", feature = "iouring", feature = "legacy"))]
        let state = match CURRENT.with(|inner| match inner {
            super::Inner::Uring(_) => None,
            super::Inner::Legacy(inner) => {
                let mut source = mio::unix::SourceFd(&fd);
                Some(super::legacy::LegacyDriver::register(
                    inner,
                    &mut source,
                    RW_INTERESTS,
                ))
            }
        }) {
            Some(reg) => State::Legacy(Some(reg?)),
            None => State::Uring(UringState::Init),
        };

        #[cfg(all(not(feature = "legacy"), target_os = "linux", feature = "iouring"))]
        let state = State::Uring(UringState::Init);

        #[cfg(all(
            unix,
            feature = "legacy",
            not(all(target_os = "linux", feature = "iouring"))
        ))]
        let state = {
            let reg = CURRENT.with(|inner| match inner {
                super::Inner::Legacy(inner) => {
                    let mut source = mio::unix::SourceFd(&fd);
                    super::legacy::LegacyDriver::register(inner, &mut source, RW_INTERESTS)
                }
            });

            State::Legacy(Some(reg?))
        };

        #[cfg(all(
            not(feature = "legacy"),
            not(all(target_os = "linux", feature = "iouring"))
        ))]
        #[allow(unused)]
        let state = super::util::feature_panic();

        #[allow(unreachable_code)]
        Ok(SharedFd {
            inner: Rc::new(Inner {
                fd,
                state: UnsafeCell::new(state),
            }),
        })
    }

    #[cfg(windows)]
    pub(crate) fn new(fd: RawHandle) -> io::Result<SharedFd> {
        unimplemented!()
    }

    #[cfg(unix)]
    #[allow(unreachable_code, unused)]
    pub(crate) fn new_without_register(fd: RawFd) -> SharedFd {
        let state = CURRENT.with(|inner| match inner {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            super::Inner::Uring(_) => State::Uring(UringState::Init),
            #[cfg(all(unix, feature = "legacy"))]
            super::Inner::Legacy(_) => State::Legacy(None),
            #[cfg(all(
                not(feature = "legacy"),
                not(all(target_os = "linux", feature = "iouring"))
            ))]
            _ => {
                super::util::feature_panic();
            }
        });

        SharedFd {
            inner: Rc::new(Inner {
                fd,
                state: UnsafeCell::new(state),
            }),
        }
    }

    #[cfg(windows)]
    #[allow(unreachable_code, unused)]
    pub(crate) fn new_without_register(fd: RawHandle) -> io::Result<SharedFd> {
        unimplemented!()
    }

    #[cfg(unix)]
    /// Returns the RawFd
    pub(crate) fn raw_fd(&self) -> RawFd {
        self.inner.fd
    }

    #[cfg(windows)]
    /// Returns the RawHandle
    pub(crate) fn raw_handle(&self) -> RawHandle {
        self.inner.fd
    }

    #[cfg(unix)]
    /// Try unwrap Rc, then deregister if registered and return rawfd.
    /// Note: this action will consume self and return rawfd without closing it.
    pub(crate) fn try_unwrap(self) -> Result<RawFd, Self> {
        let fd = self.inner.fd;
        match Rc::try_unwrap(self.inner) {
            Ok(_inner) => {
                #[cfg(all(unix, feature = "legacy"))]
                let state = unsafe { &*_inner.state.get() };

                #[cfg(all(unix, feature = "legacy"))]
                #[allow(irrefutable_let_patterns)]
                if let State::Legacy(idx) = state {
                    if CURRENT.is_set() {
                        CURRENT.with(|inner| {
                            match inner {
                                #[cfg(all(target_os = "linux", feature = "iouring"))]
                                super::Inner::Uring(_) => {
                                    unreachable!("try_unwrap legacy fd with uring runtime")
                                }
                                super::Inner::Legacy(inner) => {
                                    // deregister it from driver(Poll and slab) and close fd
                                    if let Some(idx) = idx {
                                        let mut source = mio::unix::SourceFd(&fd);
                                        let _ = super::legacy::LegacyDriver::deregister(
                                            inner,
                                            *idx,
                                            &mut source,
                                        );
                                    }
                                }
                            }
                        })
                    }
                }
                Ok(fd)
            }
            Err(inner) => Err(Self { inner }),
        }
    }

    #[cfg(windows)]
    /// Try unwrap Rc, then deregister if registered and return rawfd.
    /// Note: this action will consume self and return rawfd without closing it.
    pub(crate) fn try_unwrap(self) -> Result<RawHandle, Self> {
        unimplemented!()
    }

    #[allow(unused)]
    pub(crate) fn registered_index(&self) -> Option<usize> {
        let state = unsafe { &*self.inner.state.get() };
        match state {
            #[cfg(windows)]
            _ => unimplemented!(),
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            State::Uring(_) => None,
            #[cfg(all(unix, feature = "legacy"))]
            State::Legacy(s) => *s,
            #[cfg(all(
                not(feature = "legacy"),
                not(all(target_os = "linux", feature = "iouring"))
            ))]
            _ => {
                super::util::feature_panic();
            }
        }
    }

    /// An FD cannot be closed until all in-flight operation have completed.
    /// This prevents bugs where in-flight reads could operate on the incorrect
    /// file descriptor.
    pub(crate) async fn close(self) {
        // Here we only submit close op for uring mode.
        // Fd will be closed when Inner drops for legacy mode.
        #[cfg(all(target_os = "linux", feature = "iouring"))]
        {
            let fd = self.inner.fd;
            let mut this = self;
            #[allow(irrefutable_let_patterns)]
            if let State::Uring(uring_state) = unsafe { &mut *this.inner.state.get() } {
                if Rc::get_mut(&mut this.inner).is_some() {
                    *uring_state = match super::op::Op::close(fd) {
                        Ok(op) => UringState::Closing(op),
                        Err(_) => {
                            let _ = unsafe { std::fs::File::from_raw_fd(fd) };
                            return;
                        }
                    };
                }
                this.inner.closed().await;
            }
        }
    }
}

#[cfg(all(target_os = "linux", feature = "iouring"))]
impl Inner {
    /// Completes when the FD has been closed.
    /// Should only be called for uring mode.
    async fn closed(&self) {
        use std::task::Poll;

        crate::macros::support::poll_fn(|cx| {
            let state = unsafe { &mut *self.state.get() };

            #[allow(irrefutable_let_patterns)]
            if let State::Uring(uring_state) = state {
                use std::{future::Future, pin::Pin};

                return match uring_state {
                    UringState::Init => {
                        *uring_state = UringState::Waiting(Some(cx.waker().clone()));
                        Poll::Pending
                    }
                    UringState::Waiting(Some(waker)) => {
                        if !waker.will_wake(cx.waker()) {
                            *waker = cx.waker().clone();
                        }

                        Poll::Pending
                    }
                    UringState::Waiting(None) => {
                        *uring_state = UringState::Waiting(Some(cx.waker().clone()));
                        Poll::Pending
                    }
                    UringState::Closing(op) => {
                        // Nothing to do if the close operation failed.
                        let _ = ready!(Pin::new(op).poll(cx));
                        *uring_state = UringState::Closed;
                        Poll::Ready(())
                    }
                    UringState::Closed => Poll::Ready(()),
                };
            }
            Poll::Ready(())
        })
        .await;
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let fd = self.fd;
        let state = unsafe { &mut *self.state.get() };
        #[allow(unreachable_patterns)]
        match state {
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            State::Uring(UringState::Init) | State::Uring(UringState::Waiting(..)) => {
                if super::op::Op::close(fd).is_err() {
                    let _ = unsafe { std::fs::File::from_raw_fd(fd) };
                };
            }
            #[cfg(all(unix, feature = "legacy"))]
            State::Legacy(idx) => {
                if CURRENT.is_set() {
                    CURRENT.with(|inner| {
                        match inner {
                            #[cfg(all(target_os = "linux", feature = "iouring"))]
                            super::Inner::Uring(_) => {
                                unreachable!("close legacy fd with uring runtime")
                            }
                            #[cfg(all(unix, feature = "legacy"))]
                            super::Inner::Legacy(inner) => {
                                // deregister it from driver(Poll and slab) and close fd
                                if let Some(idx) = idx {
                                    let mut source = mio::unix::SourceFd(&fd);
                                    let _ = super::legacy::LegacyDriver::deregister(
                                        inner,
                                        *idx,
                                        &mut source,
                                    );
                                }
                            }
                        }
                    })
                }
                let _ = unsafe { std::fs::File::from_raw_fd(fd) };
            }
            // TODO: windows
            _ => {}
        }
    }
}
