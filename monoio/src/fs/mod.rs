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
