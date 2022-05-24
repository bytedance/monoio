/// Monoio Driver.
pub(crate) mod op;
pub(crate) mod shared_fd;
#[cfg(feature = "sync")]
pub(crate) mod thread;

mod legacy;
#[cfg(target_os = "linux")]
mod uring;

mod util;

use scoped_tls::scoped_thread_local;
use std::cell::UnsafeCell;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll};
use std::time::Duration;

use self::op::{CompletionMeta, Op, OpAble};

use self::legacy::LegacyInner;
#[cfg(target_os = "linux")]
use self::uring::UringInner;

pub use self::legacy::LegacyDriver;
#[cfg(target_os = "linux")]
pub use self::uring::IoUringDriver;

/// Unpark a runtime of another thread.
pub(crate) mod unpark {
    #[allow(unreachable_pub)]
    pub trait Unpark: Sync + Send + 'static {
        /// Unblocks a thread that is blocked by the associated `Park` handle.
        ///
        /// Calling `unpark` atomically makes available the unpark token, if it is
        /// not already available.
        ///
        /// # Panics
        ///
        /// This function **should** not panic, but ultimately, panics are left as
        /// an implementation detail. Refer to the documentation for the specific
        /// `Unpark` implementation
        fn unpark(&self) -> std::io::Result<()>;
    }
}

impl unpark::Unpark for Box<dyn unpark::Unpark> {
    fn unpark(&self) -> io::Result<()> {
        (**self).unpark()
    }
}

impl unpark::Unpark for std::sync::Arc<dyn unpark::Unpark> {
    fn unpark(&self) -> io::Result<()> {
        (**self).unpark()
    }
}

/// Core driver trait.
pub trait Driver {
    /// Run with driver TLS.
    fn with<R>(&self, f: impl FnOnce() -> R) -> R;
    /// Submit ops to kernel and process returned events.
    fn submit(&self) -> io::Result<()>;
    /// Wait infinitely and process returned events.
    fn park(&self) -> io::Result<()>;
    /// Wait with timeout and process returned events.
    fn park_timeout(&self, duration: Duration) -> io::Result<()>;

    /// The struct to wake thread from another.
    #[cfg(feature = "sync")]
    type Unpark: unpark::Unpark;

    /// Get Unpark.
    #[cfg(feature = "sync")]
    fn unpark(&self) -> Self::Unpark;
}

scoped_thread_local!(pub(crate) static CURRENT: Inner);

pub(crate) enum Inner {
    #[cfg(target_os = "linux")]
    Uring(Rc<UnsafeCell<UringInner>>),
    Legacy(Rc<UnsafeCell<LegacyInner>>),
}

impl Inner {
    fn submit_with<T: OpAble>(&self, data: T) -> io::Result<Op<T>> {
        match self {
            #[cfg(target_os = "linux")]
            Inner::Uring(this) => UringInner::submit_with(this, data),
            Inner::Legacy(this) => LegacyInner::submit_with(this, data),
        }
    }

    #[allow(unused)]
    fn poll_op<T: OpAble>(
        &self,
        data: &mut Pin<Box<T>>,
        index: usize,
        cx: &mut Context<'_>,
    ) -> Poll<CompletionMeta> {
        match self {
            #[cfg(target_os = "linux")]
            Inner::Uring(this) => UringInner::poll_op(this, index, cx),
            Inner::Legacy(this) => LegacyInner::poll_op::<T>(this, data, cx),
        }
    }

    #[allow(unused)]
    fn drop_op<T: 'static>(&self, index: usize, data: &mut Option<Pin<Box<T>>>) {
        match self {
            #[cfg(target_os = "linux")]
            Inner::Uring(this) => UringInner::drop_op(this, index, data),
            Inner::Legacy(_) => {}
        }
    }

    #[cfg(target_os = "linux")]
    fn is_legacy(&self) -> bool {
        matches!(self, Inner::Legacy(..))
    }

    #[allow(unused)]
    #[cfg(not(target_os = "linux"))]
    fn is_legacy(&self) -> bool {
        true
    }
}

/// The unified UnparkHandle.
#[cfg(feature = "sync")]
#[derive(Clone)]
pub(crate) enum UnparkHandle {
    #[cfg(target_os = "linux")]
    Uring(self::uring::UnparkHandle),
    Legacy(self::legacy::UnparkHandle),
}

#[cfg(feature = "sync")]
impl unpark::Unpark for UnparkHandle {
    fn unpark(&self) -> io::Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            UnparkHandle::Uring(inner) => inner.unpark(),
            UnparkHandle::Legacy(inner) => inner.unpark(),
        }
    }
}

#[cfg(all(feature = "sync", target_os = "linux"))]
impl From<self::uring::UnparkHandle> for UnparkHandle {
    fn from(inner: self::uring::UnparkHandle) -> Self {
        Self::Uring(inner)
    }
}

#[cfg(feature = "sync")]
impl From<self::legacy::UnparkHandle> for UnparkHandle {
    fn from(inner: self::legacy::UnparkHandle) -> Self {
        Self::Legacy(inner)
    }
}
