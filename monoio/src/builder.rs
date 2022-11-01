use std::{io, marker::PhantomData};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use crate::driver::IoUringDriver;
#[cfg(all(unix, feature = "legacy"))]
use crate::driver::LegacyDriver;
use crate::{
    driver::Driver,
    time::{driver::TimeDriver, Clock},
    utils::thread_id::gen_id,
    Runtime,
};

// ===== basic builder structure definition =====

/// Runtime builder
pub struct RuntimeBuilder<D> {
    // iouring entries
    entries: Option<u32>,
    // blocking handle
    #[cfg(feature = "sync")]
    blocking_handle: crate::blocking::BlockingHandle,
    // driver mark
    _mark: PhantomData<D>,
}

scoped_thread_local!(pub(crate) static BUILD_THREAD_ID: usize);

impl<T> Default for RuntimeBuilder<T> {
    /// Create a default runtime builder
    #[must_use]
    fn default() -> Self {
        RuntimeBuilder::<T>::new()
    }
}

impl<T> RuntimeBuilder<T> {
    /// Create a default runtime builder
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: None,
            #[cfg(feature = "sync")]
            blocking_handle: crate::blocking::BlockingStrategy::Panic.into(),
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

#[allow(unused)]
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

#[cfg(all(target_os = "linux", feature = "iouring"))]
direct_build!(IoUringDriver);
#[cfg(all(target_os = "linux", feature = "iouring"))]
direct_build!(TimeDriver<IoUringDriver>);
#[cfg(all(unix, feature = "legacy"))]
direct_build!(LegacyDriver);
#[cfg(all(unix, feature = "legacy"))]
direct_build!(TimeDriver<LegacyDriver>);

// ===== builder impl =====

#[cfg(all(unix, feature = "legacy"))]
impl Buildable for LegacyDriver {
    fn build(this: &RuntimeBuilder<Self>) -> io::Result<Runtime<LegacyDriver>> {
        let thread_id = gen_id();
        #[cfg(feature = "sync")]
        let blocking_handle = this.blocking_handle.clone();

        BUILD_THREAD_ID.set(&thread_id, || {
            let driver = match this.entries {
                Some(entries) => LegacyDriver::new_with_entries(entries)?,
                None => LegacyDriver::new()?,
            };
            #[cfg(feature = "sync")]
            let context = crate::runtime::Context::new(blocking_handle);
            #[cfg(not(feature = "sync"))]
            let context = crate::runtime::Context::new();
            Ok(Runtime { driver, context })
        })
    }
}

#[cfg(all(target_os = "linux", feature = "iouring"))]
impl Buildable for IoUringDriver {
    fn build(this: &RuntimeBuilder<Self>) -> io::Result<Runtime<IoUringDriver>> {
        let thread_id = gen_id();
        #[cfg(feature = "sync")]
        let blocking_handle = this.blocking_handle.clone();

        BUILD_THREAD_ID.set(&thread_id, || {
            let driver = match this.entries {
                Some(entries) => IoUringDriver::new_with_entries(entries)?,
                None => IoUringDriver::new()?,
            };
            #[cfg(feature = "sync")]
            let context = crate::runtime::Context::new(blocking_handle);
            #[cfg(not(feature = "sync"))]
            let context = crate::runtime::Context::new();
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
#[cfg(any(all(target_os = "linux", feature = "iouring"), feature = "legacy"))]
pub struct FusionDriver;

#[cfg(any(all(target_os = "linux", feature = "iouring"), feature = "legacy"))]
impl RuntimeBuilder<FusionDriver> {
    /// Build the runtime.
    #[cfg(all(target_os = "linux", feature = "iouring", feature = "legacy"))]
    pub fn build(&self) -> io::Result<crate::FusionRuntime<IoUringDriver, LegacyDriver>> {
        if crate::utils::detect_uring() {
            let builder = RuntimeBuilder::<IoUringDriver> {
                entries: self.entries,
                #[cfg(feature = "sync")]
                blocking_handle: self.blocking_handle.clone(),
                _mark: PhantomData,
            };
            info!("io_uring driver built");
            Ok(builder.build()?.into())
        } else {
            let builder = RuntimeBuilder::<LegacyDriver> {
                entries: self.entries,
                #[cfg(feature = "sync")]
                blocking_handle: self.blocking_handle.clone(),
                _mark: PhantomData,
            };
            info!("legacy driver built");
            Ok(builder.build()?.into())
        }
    }

    /// Build the runtime.
    #[cfg(all(unix, not(all(target_os = "linux", feature = "iouring"))))]
    pub fn build(&self) -> io::Result<crate::FusionRuntime<LegacyDriver>> {
        let builder = RuntimeBuilder::<LegacyDriver> {
            entries: self.entries,
            #[cfg(feature = "sync")]
            blocking_handle: self.blocking_handle.clone(),
            _mark: PhantomData,
        };
        Ok(builder.build()?.into())
    }

    /// Build the runtime.
    #[cfg(all(target_os = "linux", feature = "iouring", not(feature = "legacy")))]
    pub fn build(&self) -> io::Result<crate::FusionRuntime<IoUringDriver>> {
        let builder = RuntimeBuilder::<IoUringDriver> {
            entries: self.entries,
            #[cfg(feature = "sync")]
            blocking_handle: self.blocking_handle.clone(),
            _mark: PhantomData,
        };
        Ok(builder.build()?.into())
    }
}

#[cfg(any(all(target_os = "linux", feature = "iouring"), feature = "legacy"))]
impl RuntimeBuilder<TimeDriver<FusionDriver>> {
    /// Build the runtime.
    #[cfg(all(target_os = "linux", feature = "iouring", feature = "legacy"))]
    pub fn build(
        &self,
    ) -> io::Result<crate::FusionRuntime<TimeDriver<IoUringDriver>, TimeDriver<LegacyDriver>>> {
        if crate::utils::detect_uring() {
            let builder = RuntimeBuilder::<TimeDriver<IoUringDriver>> {
                entries: self.entries,
                #[cfg(feature = "sync")]
                blocking_handle: self.blocking_handle.clone(),
                _mark: PhantomData,
            };
            info!("io_uring driver with timer built");
            Ok(builder.build()?.into())
        } else {
            let builder = RuntimeBuilder::<TimeDriver<LegacyDriver>> {
                entries: self.entries,
                #[cfg(feature = "sync")]
                blocking_handle: self.blocking_handle.clone(),
                _mark: PhantomData,
            };
            info!("legacy driver with timer built");
            Ok(builder.build()?.into())
        }
    }

    /// Build the runtime.
    #[cfg(all(unix, not(all(target_os = "linux", feature = "iouring"))))]
    pub fn build(&self) -> io::Result<crate::FusionRuntime<TimeDriver<LegacyDriver>>> {
        let builder = RuntimeBuilder::<TimeDriver<LegacyDriver>> {
            entries: self.entries,
            #[cfg(feature = "sync")]
            blocking_handle: self.blocking_handle.clone(),
            _mark: PhantomData,
        };
        Ok(builder.build()?.into())
    }

    /// Build the runtime.
    #[cfg(all(target_os = "linux", feature = "iouring", not(feature = "legacy")))]
    pub fn build(&self) -> io::Result<crate::FusionRuntime<TimeDriver<IoUringDriver>>> {
        let builder = RuntimeBuilder::<TimeDriver<IoUringDriver>> {
            entries: self.entries,
            #[cfg(feature = "sync")]
            blocking_handle: self.blocking_handle.clone(),
            _mark: PhantomData,
        };
        Ok(builder.build()?.into())
    }
}

// ===== enable_timer related =====
mod time_wrap {
    pub trait TimeWrapable {}
}

#[cfg(all(target_os = "linux", feature = "iouring"))]
impl time_wrap::TimeWrapable for IoUringDriver {}
#[cfg(all(unix, feature = "legacy"))]
impl time_wrap::TimeWrapable for LegacyDriver {}
#[cfg(any(all(target_os = "linux", feature = "iouring"), feature = "legacy"))]
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
            #[cfg(feature = "sync")]
            blocking_handle: this.blocking_handle.clone(),
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
        let Self {
            entries,
            #[cfg(feature = "sync")]
            blocking_handle,
            ..
        } = self;
        RuntimeBuilder {
            entries,
            #[cfg(feature = "sync")]
            blocking_handle,
            _mark: PhantomData,
        }
    }

    /// Attach thread pool, this will overwrite blocking strategy.
    /// All `spawn_blocking` will be executed on given thread pool.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn attach_thread_pool(
        mut self,
        tp: std::sync::Arc<dyn crate::blocking::ThreadPool>,
    ) -> Self {
        self.blocking_handle = crate::blocking::BlockingHandle::Attached(tp);
        self
    }

    /// Set blocking strategy, this will overwrite thread pool setting.
    /// If `BlockingStrategy::Panic` is used, it will panic if `spawn_blocking` on this thread.
    /// If `BlockingStrategy::ExecuteLocal` is used, it will execute with current thread, and may
    /// cause tasks high latency.
    /// Attaching a thread pool is recommended if `spawn_blocking` will be used.
    #[cfg(feature = "sync")]
    #[must_use]
    pub fn with_blocking_strategy(mut self, strategy: crate::blocking::BlockingStrategy) -> Self {
        self.blocking_handle = crate::blocking::BlockingHandle::Empty(strategy);
        self
    }
}
