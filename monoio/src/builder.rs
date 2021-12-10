use std::{io, marker::PhantomData};

use scoped_tls::scoped_thread_local;

use crate::time::driver::TimeDriver;
use crate::{driver::IoUringDriver, runtime::Context, time::Clock, Runtime};

/// Runtime builder
pub struct RuntimeBuilder<D> {
    // iouring entries
    entries: Option<u32>,
    // driver mark
    _mark: PhantomData<D>,
}

scoped_thread_local!(pub(crate) static BUILD_THREAD_ID: usize);

impl Default for RuntimeBuilder<IoUringDriver> {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeBuilder<IoUringDriver> {
    /// Create a default runtime builder
    pub fn new() -> Self {
        Self {
            entries: None,
            _mark: PhantomData,
        }
    }

    /// Enable all(currently only timer)
    pub fn enable_all(self) -> RuntimeBuilder<TimeDriver<IoUringDriver>> {
        self.enable_timer()
    }

    /// Enable timer
    pub fn enable_timer(self) -> RuntimeBuilder<TimeDriver<IoUringDriver>> {
        let Self { entries, .. } = self;
        RuntimeBuilder {
            entries,
            _mark: PhantomData,
        }
    }

    /// Build the runtime
    pub fn build(&self) -> io::Result<Runtime<IoUringDriver>> {
        #[cfg(not(feature = "sync"))]
        let thread_id = 0;
        #[cfg(feature = "sync")]
        let thread_id = crate::utils::thread_id::gen_id();

        BUILD_THREAD_ID.set(&thread_id, || {
            let driver = match self.entries {
                Some(entries) => IoUringDriver::new_with_entries(entries)?,
                None => IoUringDriver::new()?,
            };
            let context = Context::default();
            Ok(Runtime { driver, context })
        })
    }
}

impl RuntimeBuilder<TimeDriver<IoUringDriver>> {
    /// Build the runtime
    pub fn build(&self) -> io::Result<Runtime<TimeDriver<IoUringDriver>>> {
        #[cfg(not(feature = "sync"))]
        let thread_id = 0;
        #[cfg(feature = "sync")]
        let thread_id = crate::utils::thread_id::gen_id();

        BUILD_THREAD_ID.set(&thread_id, || {
            let io_driver = match self.entries {
                Some(entries) => IoUringDriver::new_with_entries(entries)?,
                None => IoUringDriver::new()?,
            };
            let timer_driver = TimeDriver::new(io_driver, Clock::new());
            let context = Context::new_with_time_handle(timer_driver.handle.clone());
            Ok(Runtime {
                driver: timer_driver,
                context,
            })
        })
    }
}

impl<D> RuntimeBuilder<D> {
    const MIN_ENTRIES: u32 = 256;

    /// Set io_uring entries, min size is 256 and the default size is 1024.
    pub fn with_entries(mut self, entries: u32) -> Self {
        // If entries is less than 256, it will be 256.
        if entries < Self::MIN_ENTRIES {
            self.entries = Some(Self::MIN_ENTRIES);
            return self;
        }
        self.entries = Some(entries);
        self
    }
}
