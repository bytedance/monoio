use std::{
    fs::File as StdFile,
    io,
    os::fd::{AsRawFd, IntoRawFd, RawFd},
};

use super::File;
use crate::{
    buf::{IoVecBuf, IoVecBufMut},
    driver::{op::Op, shared_fd::SharedFd},
    fs::{metadata::FileAttr, Metadata},
};

impl File {
    /// Converts a [`std::fs::File`] to a [`monoio::fs::File`](File).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // This line could block. It is not recommended to do this on the monoio
    /// // runtime.
    /// let std_file = std::fs::File::open("foo.txt").unwrap();
    /// let file = monoio::fs::File::from_std(std_file);
    /// ```
    pub fn from_std(std: StdFile) -> std::io::Result<File> {
        Ok(File {
            fd: SharedFd::new_without_register(std.into_raw_fd()),
        })
    }

    /// Queries metadata about the underlying file.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs::File;
    ///
    /// #[monoio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let mut f = File::open("foo.txt").await?;
    ///     let metadata = f.metadata().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn metadata(&self) -> io::Result<Metadata> {
        metadata(self.fd.clone()).await
    }
}

impl AsRawFd for File {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

pub(crate) async fn read_vectored<T: IoVecBufMut>(
    fd: SharedFd,
    buf_vec: T,
) -> crate::BufResult<usize, T> {
    let op = Op::readv(fd, buf_vec).unwrap();
    op.result().await
}

pub(crate) async fn write_vectored<T: IoVecBuf>(
    fd: SharedFd,
    buf_vec: T,
) -> crate::BufResult<usize, T> {
    let op = Op::writev(fd, buf_vec).unwrap();
    op.result().await
}

pub(crate) async fn metadata(fd: SharedFd) -> std::io::Result<Metadata> {
    #[cfg(target_os = "linux")]
    let flags = libc::AT_STATX_SYNC_AS_STAT | libc::AT_EMPTY_PATH;
    #[cfg(target_os = "linux")]
    let op = Op::statx_using_fd(fd, flags)?;
    #[cfg(target_os = "macos")]
    let op = Op::statx_using_fd(fd, true)?;

    op.result().await.map(FileAttr::from).map(Metadata)
}
