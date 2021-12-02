//! Source of time abstraction.
//!
//! By default, `std::time::Instant::now()` is used. However, when the
//! `test-util` feature flag is enabled, the values returned for `now()` are
//! configurable.

use crate::time::Instant;

#[derive(Default, Debug, Clone)]
pub(crate) struct Clock {}

pub(crate) fn now() -> Instant {
    Instant::from_std(std::time::Instant::now())
}

impl Clock {
    pub(crate) fn new() -> Clock {
        Clock {}
    }

    pub(crate) fn now(&self) -> Instant {
        now()
    }
}
