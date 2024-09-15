use std::{io, path::Path};

use crate::driver::op::Op;

/// Creates a new symbolic link on the filesystem.
/// The dst path will be a symbolic link pointing to the src path.
/// This is an async version of std::os::unix::fs::symlink.
pub async fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(src: P, dst: Q) -> io::Result<()> {
    Op::symlink(src, dst)?.await.meta.result?;
    Ok(())
}
