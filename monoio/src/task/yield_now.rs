//! Yield the current task once to let other tasks run.
//!
//! This module provides a small utility future that cooperatively yields
//! execution back to the scheduler. It is useful when a task wants to
//! give other tasks a chance to make progress.
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// Cooperatively yield the current task.
///
/// Returns a future that yields once, allowing the scheduler to run other
/// tasks. The returned future will be `Pending` on the first poll and
/// `Ready(())` on the second.
///
/// This is similar to `tokio::task::yield_now`.
pub fn yield_now() -> YieldNow {
    YieldNow(false)
}

/// A future that completes after yielding once.
///
/// Created by [`yield_now`].
pub struct YieldNow(bool);

impl Future for YieldNow {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Second poll: complete the future.
        if self.0 {
            Poll::Ready(())
        } else {
            // First poll: request to be polled again and yield to scheduler.
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}
