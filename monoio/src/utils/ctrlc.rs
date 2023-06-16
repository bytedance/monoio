//! Forked from https://github.com/kennytm/async-ctrlc/blob/master/src/lib.rs

use std::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    ptr::null_mut,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
    task::{Context, Poll, Waker},
};

use ctrlc::set_handler;
pub use ctrlc::Error;

use crate::driver::{unpark::Unpark, UnparkHandle};

static WAKER: AtomicPtr<Waker> = AtomicPtr::new(null_mut());
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// A future which is fulfilled when the program receives the Ctrl+C signal.
#[derive(Debug)]
pub struct CtrlC {
    // Make it not Send or Sync since the signal handler holds an UnparkHandle
    // of current thread.
    // If users want to wake other threads, they should do it with channel manually.
    _private: PhantomData<*const ()>,
}

impl Future for CtrlC {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if ACTIVE.swap(false, Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            let new_waker = Box::new(cx.waker().clone());
            let old_waker_ptr = WAKER.swap(Box::into_raw(new_waker), Ordering::SeqCst);
            if !old_waker_ptr.is_null() {
                let _ = unsafe { Box::from_raw(old_waker_ptr) };
            }
            Poll::Pending
        }
    }
}

impl CtrlC {
    /// Creates a new `CtrlC` future.
    ///
    /// There should be at most one `CtrlC` instance in the whole program. The
    /// second call to `Ctrl::new()` would return an error.
    pub fn new() -> Result<Self, Error> {
        let unpark_handler = UnparkHandle::current();
        set_handler(move || {
            ACTIVE.store(true, Ordering::SeqCst);
            let waker_ptr = WAKER.swap(null_mut(), Ordering::SeqCst);
            if !waker_ptr.is_null() {
                unsafe { Box::from_raw(waker_ptr) }.wake();
            }
            let _ = unpark_handler.unpark();
        })?;
        Ok(CtrlC {
            _private: PhantomData,
        })
    }
}
