//! Monoio is a pure io_uring based rust runtime. Part of the design is borrowed
//! from tokio and tokio-uring. However, unlike tokio-uring which use uring over
//! epoll, monoio is not based on another runtime, which makes it more
//! efficient. Also, monoio is designed as thread-per-core model. Users don't
//! need to worry about Send and Sync of tasks and can use thread local storage.
//! For example, if user wants to do collect and submit tasks, he can use thread
//! local storage to avoid synchronization structures like Mutex; also, the
//! submit task will always be executed on the same thread.

#![warn(missing_docs, unreachable_pub)]
#![allow(stable_features)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(box_into_inner)]
#![feature(new_uninit)]
#![feature(io_error_more)]
#![feature(stmt_expr_attributes)]
#![feature(unboxed_closures)]
#![feature(once_cell)]

#[macro_use]
pub mod macros;
#[cfg(feature = "macros")]
#[doc(hidden)]
pub use monoio_macros::select_priv_declare_output_enum;
#[macro_use]
mod driver;
pub(crate) mod builder;
pub(crate) mod runtime;
mod scheduler;
pub mod time;

extern crate alloc;

#[cfg(feature = "sync")]
pub mod blocking;

pub mod buf;
pub mod fs;
pub mod io;
pub mod net;
pub mod task;
pub mod utils;

use std::future::Future;

#[cfg(feature = "sync")]
pub use blocking::spawn_blocking;
pub use builder::{Buildable, RuntimeBuilder};
pub use driver::Driver;
#[cfg(all(target_os = "linux", feature = "iouring"))]
pub use driver::IoUringDriver;
#[cfg(all(unix, feature = "legacy"))]
pub use driver::LegacyDriver;
#[cfg(feature = "macros")]
pub use monoio_macros::{main, test, test_all};
pub use runtime::{spawn, Runtime};
#[cfg(all(
    unix,
    any(all(target_os = "linux", feature = "iouring"), feature = "legacy")
))]
pub use {builder::FusionDriver, runtime::FusionRuntime};

/// Start a monoio runtime.
///
/// # Examples
///
/// Basic usage
///
/// ```no_run
/// use monoio::fs::File;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     monoio::start::<monoio::LegacyDriver, _>(async {
///         // Open a file
///         let file = File::open("hello.txt").await?;
///
///         let buf = vec![0; 4096];
///         // Read some data, the buffer is passed by ownership and
///         // submitted to the kernel. When the operation completes,
///         // we get the buffer back.
///         let (res, buf) = file.read_at(buf, 0).await;
///         let n = res?;
///
///         // Display the contents
///         println!("{:?}", &buf[..n]);
///
///         Ok(())
///     })
/// }
/// ```
pub fn start<D, F>(future: F) -> F::Output
where
    F: Future,
    F::Output: 'static,
    D: Buildable + Driver,
{
    let mut rt = builder::Buildable::build(&builder::RuntimeBuilder::<D>::new())
        .expect("Unable to build runtime.");
    rt.block_on(future)
}

/// A specialized `Result` type for `io-uring` operations with buffers.
///
/// This type is used as a return value for asynchronous `io-uring` methods that
/// require passing ownership of a buffer to the runtime. When the operation
/// completes, the buffer is returned whether or not the operation completed
/// successfully.
///
/// # Examples
///
/// ```no_run
/// use monoio::fs::File;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     monoio::start::<monoio::LegacyDriver, _>(async {
///         // Open a file
///         let file = File::open("hello.txt").await?;
///
///         let buf = vec![0; 4096];
///         // Read some data, the buffer is passed by ownership and
///         // submitted to the kernel. When the operation completes,
///         // we get the buffer back.
///         let (res, buf) = file.read_at(buf, 0).await;
///         let n = res?;
///
///         // Display the contents
///         println!("{:?}", &buf[..n]);
///
///         Ok(())
///     })
/// }
/// ```
pub type BufResult<T, B> = (std::io::Result<T>, B);
