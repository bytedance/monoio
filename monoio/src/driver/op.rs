use std::{
    future::Future,
    io,
    pin::Pin,
    task::{Context, Poll},
};

use crate::driver;

pub(crate) mod close;

mod accept;
mod connect;
mod fsync;
mod open;
mod poll;
mod read;
mod recv;
mod send;
mod write;

#[cfg(all(target_os = "linux", feature = "splice"))]
mod splice;

/// In-flight operation
pub(crate) struct Op<T: 'static> {
    // Driver running the operation
    pub(super) driver: driver::Inner,

    // Operation index in the slab(useless for legacy)
    pub(super) index: usize,

    // Per-operation data
    pub(super) data: Option<T>,
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

pub(crate) trait OpAble {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry;

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(super::legacy::ready::Direction, usize)>;
    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32>;
}

/// If legacy is enabled and iouring is not, we can expose io interface in a poll-like way.
/// This can provide better compatibility for crates programmed in poll-like way.
#[cfg(all(unix, feature = "legacy"))]
pub(crate) trait PollLegacy {
    fn poll_legacy(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<CompletionMeta>;
}

#[cfg(all(unix, feature = "legacy"))]
impl<T> PollLegacy for T
where
    T: OpAble,
{
    fn poll_legacy(&mut self, _cx: &mut std::task::Context<'_>) -> std::task::Poll<CompletionMeta> {
        #[cfg(all(feature = "iouring", feature = "tokio-compat"))]
        unsafe {
            extern "C" {
                #[link_name = "tokio-compat can only be enabled when legacy feature is enabled and \
                               iouring is not"]
                fn trigger() -> !;
            }
            trigger()
        }

        #[cfg(not(all(feature = "iouring", feature = "tokio-compat")))]
        driver::CURRENT.with(|this| this.poll_op(self, 0, _cx))
    }
}

impl<T> Op<T> {
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
    #[allow(unused)]
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
    T: Unpin + OpAble + 'static,
{
    type Output = Completion<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = &mut *self;
        let data_mut = me.data.as_mut().expect("unexpected operation state");
        let meta = ready!(me.driver.poll_op::<T>(data_mut, me.index, cx));

        me.index = usize::MAX;
        let data = me.data.take().expect("unexpected operation state");
        Poll::Ready(Completion { data, meta })
    }
}

impl<T> Drop for Op<T> {
    fn drop(&mut self) {
        self.driver.drop_op(self.index, &mut self.data);
    }
}

#[allow(unused)]
#[cfg(not(target_os = "linux"))]
pub(crate) fn is_legacy() -> bool {
    true
}

#[cfg(target_os = "linux")]
pub(crate) fn is_legacy() -> bool {
    super::CURRENT.with(|inner| inner.is_legacy())
}
