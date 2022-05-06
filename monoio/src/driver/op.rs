use crate::driver;

use io_uring::squeue;
use std::cell::UnsafeCell;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

pub(crate) mod close;

mod accept;
mod connect;
mod fsync;
mod open;
mod read;
mod recv;
mod send;
mod write;

/// In-flight operation
pub(crate) struct Op<T: 'static> {
    // Driver running the operation
    pub(super) driver: Rc<UnsafeCell<driver::Inner>>,

    // Operation index in the slab
    pub(super) index: usize,

    // Per-operation data
    data: Option<Pin<Box<T>>>,
}

/// Operation completion. Returns stored state with the result of the operation.
#[derive(Debug)]
pub(crate) struct Completion<T> {
    pub(crate) data: T,
    pub(crate) result: io::Result<u32>,
    #[allow(unused)]
    pub(crate) flags: u32,
}

pub(crate) enum Lifecycle {
    /// The operation has been submitted to uring and is currently in-flight
    Submitted,

    /// The submitter is waiting for the completion of the operation
    Waiting(Waker),

    /// The submitter no longer has interest in the operation result. The state
    /// must be passed to the driver and held until the operation completes.
    Ignored(Box<dyn std::any::Any>),

    /// The operation has completed.
    Completed(io::Result<u32>, u32),
}

impl<T> Op<T> {
    /// Create a new operation
    fn new(data: T, inner: &mut driver::Inner, inner_rc: &Rc<UnsafeCell<driver::Inner>>) -> Op<T> {
        Op {
            driver: inner_rc.clone(),
            index: inner.ops.insert(),
            data: Some(Box::pin(data)),
        }
    }

    /// Submit an operation to uring.
    ///
    /// `state` is stored during the operation tracking any state submitted to
    /// the kernel.
    pub(super) fn submit_with<F>(data: T, f: F) -> io::Result<Op<T>>
    where
        F: FnOnce(&mut Pin<Box<T>>) -> squeue::Entry,
    {
        driver::CURRENT.with(|inner_rc| {
            let inner = unsafe { &mut *inner_rc.get() };

            // If the submission queue is full, flush it to the kernel
            if inner.uring.submission().is_full() {
                inner.submit()?;
            }

            // Create the operation
            let mut op = Op::new(data, inner, inner_rc);

            // Configure the SQE
            let sqe = f(unsafe { op.data.as_mut().unwrap_unchecked() }).user_data(op.index as _);

            {
                let mut sq = inner.uring.submission();

                // Push the new operation
                if unsafe { sq.push(&sqe).is_err() } {
                    unimplemented!("when is this hit?");
                }
            }

            // Submit the new operation. At this point, the operation has been
            // pushed onto the queue and the tail pointer has been updated, so
            // the submission entry is visible to the kernel. If there is an
            // error here (probably EAGAIN), we still return the operation. A
            // future `io_uring_enter` will fully submit the event.

            // CHIHAI: We are not going to do syscall now. If we are waiting
            // for IO, we will submit on `park`.
            // let _ = inner.submit();
            Ok(op)
        })
    }

    /// Try submitting an operation to uring
    pub(super) fn try_submit_with<F>(data: T, f: F) -> io::Result<Op<T>>
    where
        F: FnOnce(&mut Pin<Box<T>>) -> squeue::Entry,
    {
        if driver::CURRENT.is_set() {
            Op::submit_with(data, f)
        } else {
            Err(io::ErrorKind::Other.into())
        }
    }
}

impl<T> Future for Op<T>
where
    T: Unpin + 'static,
{
    type Output = Completion<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        let inner = unsafe { &mut *me.driver.get() };

        let lifecycle = unsafe { inner.ops.slab.get_mut(me.index).unwrap_unchecked() };
        match lifecycle {
            Lifecycle::Submitted => {
                *lifecycle = Lifecycle::Waiting(cx.waker().clone());
                return Poll::Pending;
            }
            Lifecycle::Waiting(waker) => {
                if !waker.will_wake(cx.waker()) {
                    *lifecycle = Lifecycle::Waiting(cx.waker().clone());
                }
                return Poll::Pending;
            }
            _ => {}
        }

        match unsafe { inner.ops.slab.remove(me.index).unwrap_unchecked() } {
            Lifecycle::Completed(result, flags) => {
                me.index = usize::MAX;
                let pinned_data = me.data.take().expect("unexpected operation state");
                let data = Box::into_inner(unsafe { Pin::into_inner_unchecked(pinned_data) });
                Poll::Ready(Completion {
                    data,
                    result,
                    flags,
                })
            }
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
    }
}

impl<T> Drop for Op<T> {
    fn drop(&mut self) {
        let inner = unsafe { &mut *self.driver.get() };
        let lifecycle = match inner.ops.slab.get_mut(self.index) {
            Some(lifecycle) => lifecycle,
            None => return,
        };

        match lifecycle {
            Lifecycle::Submitted | Lifecycle::Waiting(_) => {
                *lifecycle = Lifecycle::Ignored(Box::new(self.data.take()));
                #[cfg(features = "async-cancel")]
                unsafe {
                    let cancel = io_uring::opcode::AsyncCancel::new(self.index as u64).build();

                    // Try push cancel, if failed, will submit and re-push.
                    if inner.uring.submission().push(&cancel).is_err() {
                        let _ = inner.submit();
                        let _ = inner.uring.submission().push(&cancel);
                    }
                }
            }
            Lifecycle::Completed(..) => {
                inner.ops.slab.remove(self.index);
            }
            Lifecycle::Ignored(..) => unreachable!(),
        }
    }
}
