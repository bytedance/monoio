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

mod open_options;
pub use open_options::OpenOptions;

#[cfg(unix)]
mod metadata;
#[cfg(unix)]
pub use metadata::{metadata, symlink_metadata, Metadata};

#[cfg(unix)]
mod file_type;
#[cfg(target_os = "linux")]
pub use file_type::FileType;

#[cfg(unix)]
mod permissions;
#[cfg(target_os = "linux")]
pub use permissions::Permissions;

use crate::buf::IoBuf;
#[cfg(all(unix, feature = "unlinkat"))]
use crate::driver::op::Op;

/// Read the entire contents of a file into a bytes vector.
#[cfg(unix)]
pub async fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};

    use crate::buf::IoBufMut;

    let file = File::open(path).await?;
    let sys_file = unsafe { std::fs::File::from_raw_fd(file.as_raw_fd()) };
    let size = sys_file.metadata()?.len() as usize;
    let _ = sys_file.into_raw_fd();

    let (res, buf) = file
        .read_exact_at(Vec::with_capacity(size).slice_mut(0..size), 0)
        .await;
    res?;
    Ok(buf.into_inner())
}

/// Write a buffer as the entire contents of a file.
pub async fn write<P: AsRef<Path>, C: IoBuf>(path: P, contents: C) -> (io::Result<()>, C) {
    let file = match File::create(path).await {
        Ok(f) => f,
        Err(e) => return (Err(e), contents),
    };
    file.write_all_at(contents, 0).await
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
/// use monoio::fs::File;
///
/// #[monoio::main]
/// async fn main() -> std::io::Result<()> {
///     fs::remove_file("a.txt").await?;
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
/// use monoio::fs::File;
///
/// #[monoio::main]
/// async fn main() -> std::io::Result<()> {
///     fs::remove_dir("/some/dir").await?;
///     Ok(())
/// }
/// ```
#[cfg(all(unix, feature = "unlinkat"))]
pub async fn remove_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    Op::rmdir(path)?.await.meta.result?;
    Ok(())
}
