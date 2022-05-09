use crate::driver;

use io_uring::squeue;
use std::future::Future;
use std::io;
use std::pin::Pin;
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
    pub(super) driver: driver::Inner,

    // Operation index in the slab
    pub(super) index: usize,

    // Per-operation data
    pub(super) data: Option<Pin<Box<T>>>,
}

/// Operation completion. Returns stored state with the result of the operation.
#[derive(Debug)]
pub(crate) struct Completion<T> {
    pub(crate) data: T,
    pub(crate) meta: CompletionMeta,
}

/// Operation completion meta info.
#[derive(Debug)]
pub(crate) struct CompletionMeta {
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

pub(crate) trait OpAble {
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> squeue::Entry;
}

impl<T> Op<T> {
    /// Create a new operation
    pub(super) fn new(data: T, inner: &mut driver::UringInner, driver: driver::Inner) -> Op<T> {
        Op {
            driver,
            index: inner.ops.insert(),
            data: Some(Box::pin(data)),
        }
    }

    /// Submit an operation to uring.
    ///
    /// `state` is stored during the operation tracking any state submitted to
    /// the kernel.
    pub(super) fn submit_with(data: T) -> io::Result<Op<T>>
    where
        T: OpAble,
    {
        driver::CURRENT.with(|this| this.submit_with(data))
    }

    /// Try submitting an operation to uring
    pub(super) fn try_submit_with(data: T) -> io::Result<Op<T>>
    where
        T: OpAble,
    {
        if driver::CURRENT.is_set() {
            Op::submit_with(data)
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
        let meta = ready!(me.driver.poll_op(me.index, cx));

        me.index = usize::MAX;
        let pinned_data = me.data.take().expect("unexpected operation state");
        let data = Box::into_inner(unsafe { Pin::into_inner_unchecked(pinned_data) });
        Poll::Ready(Completion { data, meta })
    }
}

impl<T> Drop for Op<T> {
    fn drop(&mut self) {
        self.driver.drop(self.index, &mut self.data);
    }
}
