use super::{
    legacy::LegacyDriver,
    op::{close::Close, Op},
    CURRENT,
};

use futures::future::poll_fn;

use std::cell::UnsafeCell;
use std::io;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::rc::Rc;
use std::task::Waker;

// Tracks in-flight operations on a file descriptor. Ensures all in-flight
// operations complete before submitting the close.
#[derive(Clone, Debug)]
pub(crate) struct SharedFd {
    inner: Rc<Inner>,
}

struct Inner {
    // Open file descriptor
    fd: RawFd,

    // Waker to notify when the close operation completes.
    state: UnsafeCell<State>,
}

enum State {
    Uring(UringState),
    Legacy(Option<usize>),
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Inner").field("fd", &self.fd).finish()
    }
}

enum UringState {
    /// Initial state
    Init,

    /// Waiting for all in-flight operation to complete.
    Waiting(Option<Waker>),

    /// The FD is closing
    Closing(Op<Close>),

    /// The FD is fully closed
    Closed,
}

impl AsRawFd for SharedFd {
    fn as_raw_fd(&self) -> RawFd {
        self.raw_fd()
    }
}

impl SharedFd {
    pub(crate) fn new(fd: RawFd) -> io::Result<SharedFd> {
        let state = match CURRENT.with(|inner| {
            const RW_INTERESTS: mio::Interest =
                mio::Interest::READABLE.add(mio::Interest::WRITABLE);

            match inner {
                #[cfg(target_os = "linux")]
                super::Inner::Uring(_) => None,
                super::Inner::Legacy(inner) => {
                    let mut source = mio::unix::SourceFd(&fd);
                    Some(LegacyDriver::register(inner, &mut source, RW_INTERESTS))
                }
            }
        }) {
            Some(reg) => State::Legacy(Some(reg?)),
            None => State::Uring(UringState::Init),
        };

        Ok(SharedFd {
            inner: Rc::new(Inner {
                fd,
                state: UnsafeCell::new(state),
            }),
        })
    }

    pub(crate) fn new_without_register(fd: RawFd) -> io::Result<SharedFd> {
        let state = CURRENT.with(|inner| match inner {
            #[cfg(target_os = "linux")]
            super::Inner::Uring(_) => State::Uring(UringState::Init),
            super::Inner::Legacy(_) => State::Legacy(None),
        });

        Ok(SharedFd {
            inner: Rc::new(Inner {
                fd,
                state: UnsafeCell::new(state),
            }),
        })
    }

    /// Returns the RawFd
    pub(crate) fn raw_fd(&self) -> RawFd {
        self.inner.fd
    }

    pub(crate) fn registered_index(&self) -> Option<usize> {
        let state = unsafe { &*self.inner.state.get() };
        match state {
            State::Uring(_) => None,
            State::Legacy(s) => *s,
        }
    }

    /// An FD cannot be closed until all in-flight operation have completed.
    /// This prevents bugs where in-flight reads could operate on the incorrect
    /// file descriptor.
    pub(crate) async fn close(mut self) {
        let fd = self.inner.fd;
        // Here we only submit close op for uring mode.
        // Fd will be closed when Inner drops for legacy mode.
        if let State::Uring(uring_state) = unsafe { &mut *self.inner.state.get() } {
            if Rc::get_mut(&mut self.inner).is_some() {
                *uring_state = match Op::close(fd) {
                    Ok(op) => UringState::Closing(op),
                    Err(_) => {
                        let _ = unsafe { std::fs::File::from_raw_fd(fd) };
                        return;
                    }
                };
            }
            self.inner.closed().await;
        }
    }
}

impl Inner {
    /// Completes when the FD has been closed.
    /// Should only be called for uring mode.
    async fn closed(&self) {
        use std::future::Future;
        use std::pin::Pin;
        use std::task::Poll;

        poll_fn(|cx| {
            let state = unsafe { &mut *self.state.get() };
            let uring_state = match state {
                State::Uring(uring) => uring,
                State::Legacy(_) => {
                    return Poll::Ready(());
                }
            };

            match uring_state {
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
            }
        })
        .await;
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let fd = self.fd;
        let state = unsafe { &mut *self.state.get() };
        match state {
            State::Uring(UringState::Init) | State::Uring(UringState::Waiting(..)) => {
                if Op::close(fd).is_err() {
                    let _ = unsafe { std::fs::File::from_raw_fd(fd) };
                };
            }
            State::Legacy(idx) => {
                if CURRENT.is_set() {
                    CURRENT.with(|inner| {
                        match inner {
                            #[cfg(target_os = "linux")]
                            super::Inner::Uring(_) => {
                                unreachable!("close legacy fd with uring runtime")
                            }
                            super::Inner::Legacy(inner) => {
                                // deregister it from driver(Poll and slab) and close fd
                                if let Some(idx) = idx {
                                    let mut source = mio::unix::SourceFd(&fd);
                                    let _ = LegacyDriver::deregister(inner, *idx, &mut source);
                                }
                            }
                        }
                    })
                }
                let _ = unsafe { std::fs::File::from_raw_fd(fd) };
            }
            _ => {}
        }
    }
}
