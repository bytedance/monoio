use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

use super::raw::RawTask;

/// JoinHandle
pub struct JoinHandle<T> {
    raw: Option<RawTask>,
    _p: PhantomData<T>,
}

impl<T> JoinHandle<T> {
    pub(super) fn new(raw: RawTask) -> JoinHandle<T> {
        JoinHandle {
            raw: Some(raw),
            _p: PhantomData,
        }
    }
}

impl<T> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut ret = Poll::Pending;

        // Raw should always be set. If it is not, this is due to polling after
        // completion
        let raw = self
            .raw
            .as_ref()
            .expect("polling after `JoinHandle` already completed");

        // Try to read the task output. If the task is not yet complete, the
        // waker is stored and is notified once the task does complete.
        //
        // The function must go via the vtable, which requires erasing generic
        // types. To do this, the function "return" is placed on the stack
        // **before** calling the function and is passed into the function using
        // `*mut ()`.
        //
        // Safety:
        //
        // The type of `T` must match the task's output type.
        unsafe {
            raw.try_read_output(&mut ret as *mut _ as *mut (), cx.waker());
        }
        ret
    }
}

impl<T> Drop for JoinHandle<T> {
    fn drop(&mut self) {
        if let Some(raw) = self.raw.take() {
            if raw.header().state.drop_join_handle_fast().is_ok() {
                return;
            }

            raw.drop_join_handle_slow();
        }
    }
}
