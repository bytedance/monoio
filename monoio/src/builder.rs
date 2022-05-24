use std::{io, marker::PhantomData};

use scoped_tls::scoped_thread_local;

use crate::driver::Driver;
use crate::time::driver::TimeDriver;
use crate::FusionRuntime;
use crate::{driver::LegacyDriver, runtime::Context, time::Clock, Runtime};

#[cfg(target_os = "linux")]
use crate::driver::IoUringDriver;

// ===== basic builder structure definition =====

/// Runtime builder
pub struct RuntimeBuilder<D> {
    // iouring entries
    entries: Option<u32>,
    // driver mark
    _mark: PhantomData<D>,
}

scoped_thread_local!(pub(crate) static BUILD_THREAD_ID: usize);

impl<T> Default for RuntimeBuilder<T> {
    /// Create a default runtime builder
    #[must_use]
    fn default() -> Self {
        Self {
            entries: None,
            _mark: PhantomData,
        }
    }
}

impl<T> RuntimeBuilder<T> {
    /// Create a default runtime builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: None,
            _mark: PhantomData,
        }
    }
}

// ===== buildable trait and forward methods =====

/// Buildable trait.
pub trait Buildable: Sized {
    /// Build the runtime.
    fn build(this: &RuntimeBuilder<Self>) -> io::Result<Runtime<Self>>;
}

macro_rules! direct_build {
    ($ty: ty) => {
        impl RuntimeBuilder<$ty> {
            /// Build the runtime.
            pub fn build(&self) -> io::Result<Runtime<$ty>> {
                Buildable::build(self)
            }
        }
    };
}

#[cfg(target_os = "linux")]
direct_build!(IoUringDriver);
#[cfg(target_os = "linux")]
direct_build!(TimeDriver<IoUringDriver>);
direct_build!(LegacyDriver);
direct_build!(TimeDriver<LegacyDriver>);

// ===== builder impl =====

impl Buildable for LegacyDriver {
    fn build(this: &RuntimeBuilder<Self>) -> io::Result<Runtime<LegacyDriver>> {
        #[cfg(not(feature = "sync"))]
        let thread_id = 0;
        #[cfg(feature = "sync")]
        let thread_id = crate::utils::thread_id::gen_id();

        BUILD_THREAD_ID.set(&thread_id, || {
            let driver = match this.entries {
                Some(entries) => LegacyDriver::new_with_entries(entries)?,
                None => LegacyDriver::new()?,
            };
            let context = Context::default();
            Ok(Runtime { driver, context })
        })
    }
}

#[cfg(target_os = "linux")]
impl Buildable for IoUringDriver {
    fn build(this: &RuntimeBuilder<Self>) -> io::Result<Runtime<IoUringDriver>> {
        #[cfg(not(feature = "sync"))]
        let thread_id = 0;
        #[cfg(feature = "sync")]
        let thread_id = crate::utils::thread_id::gen_id();

        BUILD_THREAD_ID.set(&thread_id, || {
            let driver = match this.entries {
                Some(entries) => IoUringDriver::new_with_entries(entries)?,
                None => IoUringDriver::new()?,
            };
            let context = Context::default();
            Ok(Runtime { driver, context })
        })
    }
}

impl<D> RuntimeBuilder<D> {
    const MIN_ENTRIES: u32 = 256;

    /// Set io_uring entries, min size is 256 and the default size is 1024.
    #[must_use]
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

// ===== FusionDriver =====

/// Fake driver only for conditionally building.
pub struct FusionDriver;

impl RuntimeBuilder<FusionDriver> {
    /// Build the runtime.
    #[cfg(target_os = "linux")]
    pub fn build(&self) -> io::Result<FusionRuntime<IoUringDriver, LegacyDriver>> {
        if crate::utils::detect_uring() {
            let builder = RuntimeBuilder::<IoUringDriver> {
                entries: self.entries,
                _mark: PhantomData,
            };
            Ok(builder.build()?.into())
        } else {
            let builder = RuntimeBuilder::<LegacyDriver> {
                entries: self.entries,
                _mark: PhantomData,
            };
            Ok(builder.build()?.into())
        }
    }

    /// Build the runtime.
    #[cfg(not(target_os = "linux"))]
    pub fn build(&self) -> io::Result<FusionRuntime<LegacyDriver>> {
        let builder = RuntimeBuilder::<LegacyDriver> {
            entries: self.entries,
            _mark: PhantomData,
        };
        Ok(builder.build()?.into())
    }
}

impl RuntimeBuilder<TimeDriver<FusionDriver>> {
    /// Build the runtime.
    #[cfg(target_os = "linux")]
    pub fn build(
        &self,
    ) -> io::Result<FusionRuntime<TimeDriver<IoUringDriver>, TimeDriver<LegacyDriver>>> {
        if crate::utils::detect_uring() {
            let builder = RuntimeBuilder::<TimeDriver<IoUringDriver>> {
                entries: self.entries,
                _mark: PhantomData,
            };
            Ok(builder.build()?.into())
        } else {
            let builder = RuntimeBuilder::<TimeDriver<LegacyDriver>> {
                entries: self.entries,
                _mark: PhantomData,
            };
            Ok(builder.build()?.into())
        }
    }

    /// Build the runtime.
    #[cfg(not(target_os = "linux"))]
    pub fn build(&self) -> io::Result<FusionRuntime<TimeDriver<LegacyDriver>>> {
        let builder = RuntimeBuilder::<TimeDriver<LegacyDriver>> {
            entries: self.entries,
            _mark: PhantomData,
        };
        Ok(builder.build()?.into())
    }
}

// ===== enable_timer related =====
mod time_wrap {
    pub trait TimeWrapable {}
}

#[cfg(target_os = "linux")]
impl time_wrap::TimeWrapable for IoUringDriver {}
impl time_wrap::TimeWrapable for LegacyDriver {}
impl time_wrap::TimeWrapable for FusionDriver {}

impl<D: Driver> Buildable for TimeDriver<D>
where
    D: Buildable,
{
    /// Build the runtime
    fn build(this: &RuntimeBuilder<Self>) -> io::Result<Runtime<TimeDriver<D>>> {
        let Runtime {
            driver,
            mut context,
        } = Buildable::build(&RuntimeBuilder::<D> {
            entries: this.entries,
            _mark: PhantomData,
        })?;

        let timer_driver = TimeDriver::new(driver, Clock::new());
        context.time_handle = Some(timer_driver.handle.clone());
        Ok(Runtime {
            driver: timer_driver,
            context,
        })
    }
}

impl<D: time_wrap::TimeWrapable> RuntimeBuilder<D> {
    /// Enable all(currently only timer)
    #[must_use]
    pub fn enable_all(self) -> RuntimeBuilder<TimeDriver<D>> {
        self.enable_timer()
    }

    /// Enable timer
    #[must_use]
    pub fn enable_timer(self) -> RuntimeBuilder<TimeDriver<D>> {
        let Self { entries, .. } = self;
        RuntimeBuilder {
            entries,
            _mark: PhantomData,
        }
    }
}
