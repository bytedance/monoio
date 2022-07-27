use std::{fmt, rc::Rc};

use crate::time::driver::ClockTime;

/// Handle to time driver instance.
#[derive(Clone)]
pub(crate) struct Handle {
    time_source: ClockTime,
    inner: Rc<super::Inner>,
}

impl Handle {
    /// Creates a new timer `Handle` from a shared `Inner` timer state.
    pub(super) fn new(inner: Rc<super::Inner>) -> Self {
        let time_source = inner.state.borrow_mut().time_source.clone();
        Handle { time_source, inner }
    }

    /// Returns the time source associated with this handle
    pub(super) fn time_source(&self) -> &ClockTime {
        &self.time_source
    }

    /// Access the driver's inner structure
    pub(super) fn get(&self) -> &super::Inner {
        &self.inner
    }
}

impl Handle {
    /// Tries to get a handle to the current timer.
    ///
    /// # Panics
    ///
    /// This function panics if there is no current timer set.
    ///
    /// It can be triggered when `Builder::enable_timer()` or
    /// `Builder::enable_all()` are not included in the builder.
    ///
    /// It can also panic whenever a timer is created outside of a
    /// Monoio runtime. That is why `rt.block_on(delay_for(...))` will panic,
    /// since the function is executed outside of the runtime.
    /// Whereas `rt.block_on(async {delay_for(...).await})` doesn't panic.
    /// And this is because wrapping the function on an async makes it lazy,
    /// and so gets executed inside the runtime successfully without
    /// panicking.
    pub(crate) fn current() -> Self {
        crate::runtime::CURRENT.with(|c| {
            c.time_handle.clone().expect(
                "unable to get time handle, maybe you have not enable_timer on creating runtime?",
            )
        })
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle")
    }
}
