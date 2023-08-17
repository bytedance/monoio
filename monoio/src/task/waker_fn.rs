use core::task::{RawWaker, RawWakerVTable, Waker};
use std::cell::Cell;

/// Creates a waker that does nothing.
///
/// This `Waker` is useful for polling a `Future` to check whether it is
/// `Ready`, without doing any additional work.
pub(crate) fn dummy_waker() -> Waker {
    fn raw_waker() -> RawWaker {
        // the pointer is never dereferenced, so null is ok
        RawWaker::new(std::ptr::null::<()>(), vtable())
    }

    fn vtable() -> &'static RawWakerVTable {
        &RawWakerVTable::new(
            |_| raw_waker(),
            |_| {
                set_poll();
            },
            |_| {
                set_poll();
            },
            |_| {},
        )
    }

    unsafe { Waker::from_raw(raw_waker()) }
}

#[thread_local]
static SHOULD_POLL: Cell<bool> = Cell::new(true);

#[inline]
pub(crate) fn should_poll() -> bool {
    SHOULD_POLL.replace(false)
}

#[inline]
pub(crate) fn set_poll() {
    SHOULD_POLL.set(true);
}
