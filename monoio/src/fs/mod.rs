//! Filesystem manipulation operations.

mod file;
use std::{io, path::Path};

pub use file::File;

#[cfg(all(unix, feature = "mkdirat"))]
mod dir_builder;
#[cfg(all(unix, feature = "mkdirat"))]
pub use dir_builder::DirBuilder;

#[cfg(all(unix, feature = "mkdirat"))]
mod create_dir;
#[cfg(all(unix, feature = "mkdirat"))]
pub use create_dir::*;

#[cfg(all(unix, feature = "symlinkat"))]
mod symlink;
#[cfg(all(unix, feature = "symlinkat"))]
pub use symlink::symlink;

mod open_options;
pub use open_options::OpenOptions;

#[cfg(unix)]
mod metadata;
#[cfg(unix)]
pub use metadata::{metadata, symlink_metadata, Metadata};

#[cfg(unix)]
mod file_type;
#[cfg(unix)]
pub use file_type::FileType;

#[cfg(unix)]
mod permissions;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, IntoRawHandle};

#[cfg(unix)]
pub use permissions::Permissions;

use crate::buf::IoBuf;
#[cfg(all(unix, feature = "unlinkat"))]
use crate::driver::op::Op;

/// Executes a blocking operation asynchronously on a separate thread.
///
/// This function is designed to offload blocking I/O or CPU-bound tasks to a separate
/// thread using `spawn_blocking`, which allows non-blocking async code to continue executing.
///
/// # Parameters
///
/// * `f`: A closure or function that performs the blocking operation. This function takes no
///   arguments and returns an `io::Result<T>`. The closure must be `Send` and `'static` to ensure
///   it can be safely run in a different thread.
///
/// # Returns
///
/// This function returns an `io::Result<T>`, where `T` is the type returned by the
/// blocking operation. If the blocking task completes successfully, the result will
/// be `Ok(T)`. If the background task fails, an `io::Error` with `io::ErrorKind::Other`
/// will be returned.
///
/// # Errors
///
/// The function may return an `io::Error` in the following scenarios:
/// - The blocking task returned an error, in which case the error is propagated.
/// - The background task failed to complete due to an internal error, in which case an error with
///   `io::ErrorKind::Other` is returned.
#[cfg(all(feature = "sync", not(feature = "iouring")))]
pub(crate) async fn asyncify<F, T>(f: F) -> io::Result<T>
where
    F: FnOnce() -> io::Result<T> + Send + 'static,
    T: Send + 'static,
{
    use crate::spawn_blocking;

    match spawn_blocking(f).await {
        Ok(res) => res,
        Err(_) => Err(io::Error::new(
            io::ErrorKind::Other,
            "background task failed",
        )),
    }
}

/// A macro that generates an asynchronous I/O operation function, offloading a blocking
/// system call to a separate thread using the `asyncify` function.
///
/// This macro is intended to abstract the process of creating asynchronous functions for
/// operations that involve reading or writing buffers, making it easier to create functions
/// that perform system-level I/O asynchronously.
#[cfg(all(feature = "sync", not(feature = "iouring")))]
#[macro_export]
macro_rules! asyncify_op {
    ($fn_name:ident<$Trait: ident>($read_op:expr, $buf_ptr_expr:expr, $len_expr:expr $(, $extra_param:ident : $typ: ty)?) $(,)?) => {
        pub(crate) async fn $fn_name<T: $Trait>(
            fd: SharedFd,
            mut buf: T,
            $($extra_param: $typ)?
        ) -> $crate::BufResult<usize, T> {
            #[cfg(unix)]
            let fd = fd.as_raw_fd();
            #[cfg(windows)]
            let fd = fd.as_raw_handle() as _;
            // Safety: Due to the trait `IoBuf*/IoVecBuf*` require the implemet of `*_ptr`
            // should return the same address, it should be safe to convert it to `usize`
            // and then convert back.
            let buf_ptr = $buf_ptr_expr(&mut buf) as usize;
            let len = $len_expr(&mut buf);

            let res = $crate::fs::asyncify(move || $read_op(fd, buf_ptr as *mut _, len, $($extra_param)?))
                .await
                .map(|n| n as usize);

            unsafe { buf.set_init(*res.as_ref().unwrap_or(&0)) };

            (res, buf)
        }
    }
}

/// Read the entire contents of a file into a bytes vector.
pub async fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    use crate::buf::IoBufMut;

    let file = File::open(path).await?;

    #[cfg(windows)]
    let sys_file = unsafe { std::fs::File::from_raw_handle(file.as_raw_handle()) };
    #[cfg(windows)]
    let size = sys_file.metadata()?.len() as usize;
    #[cfg(windows)]
    let _ = sys_file.into_raw_handle();

    #[cfg(unix)]
    let size = file.metadata().await?.len() as usize;

    let (res, buf) = file
        .read_exact_at(Vec::with_capacity(size).slice_mut(0..size), 0)
        .await;
    res?;
    Ok(buf.into_inner())
}

/// Write a buffer as the entire contents of a file.
pub async fn write<P: AsRef<Path>, C: IoBuf>(path: P, contents: C) -> (io::Result<()>, C) {
    match File::create(path).await {
        Ok(f) => f.write_all_at(contents, 0).await,
        Err(e) => (Err(e), contents),
    }
}

/// Removes a file from the filesystem.
///
/// Note that there is no
/// guarantee that the file is immediately deleted (e.g., depending on
/// platform, other open file descriptors may prevent immediate removal).
///
/// # Platform-specific behavior
///
/// This function is currently only implemented for Unix.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `path` points to a directory.
/// * The file doesn't exist.
/// * The user lacks permissions to remove the file.
///
/// # Examples
///
/// ```no_run
/// #[monoio::main]
/// async fn main() -> std::io::Result<()> {
///     monoio::fs::remove_file("a.txt").await?;
///     Ok(())
/// }
/// ```
#[cfg(all(unix, feature = "unlinkat"))]
pub async fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    Op::unlink(path)?.await.meta.result?;
    Ok(())
}

/// Removes an empty directory.
///
/// # Platform-specific behavior
///
/// This function is currently only implemented for Unix.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `path` doesn't exist.
/// * `path` isn't a directory.
/// * The user lacks permissions to remove the directory at the provided `path`.
/// * The directory isn't empty.
///
/// # Examples
///
/// ```no_run
/// #[monoio::main]
/// async fn main() -> std::io::Result<()> {
///     monoio::fs::remove_dir("/some/dir").await?;
///     Ok(())
/// }
/// ```
#[cfg(all(unix, feature = "unlinkat"))]
pub async fn remove_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    Op::rmdir(path)?.await.meta.result?;
    Ok(())
}

/// Rename a file or directory to a new name, replacing the original file if
/// `to` already exists.
///
/// This will not work if the new name is on a different mount point.
///
/// This is async version of [std::fs::rename].
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * `from` does not exist.
/// * The user lacks permissions to view contents.
/// * `from` and `to` are on separate filesystems.
///
/// # Examples
///
/// ```no_run
/// #[monoio::main]
/// async fn main() -> std::io::Result<()> {
///     monoio::fs::rename("a.txt", "b.txt").await?; // Rename a.txt to b.txt
///     Ok(())
/// }
/// ```
#[cfg(all(unix, feature = "renameat"))]
pub async fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    Op::rename(from.as_ref(), to.as_ref())?.await.meta.result?;
    Ok(())
}
