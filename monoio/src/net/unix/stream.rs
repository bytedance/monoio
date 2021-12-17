use std::{
    io,
    os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
    path::Path,
};

use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    driver::{Op, SharedFd},
    io::{AsyncReadRent, AsyncWriteRent},
};

use super::socket_addr::{local_addr, pair, peer_addr, socket_addr, SocketAddr};

/// UnixStream
pub struct UnixStream {
    fd: SharedFd,
}

impl UnixStream {
    pub(crate) fn from_shared_fd(fd: SharedFd) -> Self {
        Self { fd }
    }

    /// Connect UnixStream to a path.
    pub async fn connect<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let (addr, addr_len) = socket_addr(path.as_ref())?;

        let op = Op::connect_unix(addr, addr_len)?;
        let completion = op.await;
        completion.result?;

        let stream = UnixStream::from_shared_fd(completion.data.fd);
        Ok(stream)
    }

    /// Creates an unnamed pair of connected sockets.
    ///
    /// Returns two `UnixStream`s which are connected to each other.
    pub fn pair() -> io::Result<(UnixStream, UnixStream)> {
        pair(libc::SOCK_STREAM).map(|(stream1, stream2)| {
            (UnixStream::from_std(stream1), UnixStream::from_std(stream2))
        })
    }

    /// Creates new `UnixStream` from a `std::os::unix::net::UnixStream`.
    pub fn from_std(stream: std::os::unix::net::UnixStream) -> Self {
        let fd = stream.into_raw_fd();
        unsafe { Self::from_raw_fd(fd) }
    }

    /// Returns the socket address of the local half of this connection.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        local_addr(self.as_raw_fd())
    }

    /// Returns the socket address of the remote half of this connection.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        peer_addr(self.as_raw_fd())
    }
}

impl std::os::unix::io::FromRawFd for UnixStream {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self::from_shared_fd(SharedFd::new(fd))
    }
}

impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl AsyncWriteRent for UnixStream {
    type WriteFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type WritevFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ShutdownFuture<'a> = impl std::future::Future<Output = Result<(), std::io::Error>>;

    fn write<T: IoBuf>(&self, buf: T) -> Self::WriteFuture<'_, T> {
        // Submit the write operation
        let op = Op::send(&self.fd, buf).unwrap();
        op.write()
    }

    fn writev<T: IoVecBuf>(&self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        let op = Op::writev(&self.fd, buf_vec).unwrap();
        op.write()
    }

    fn shutdown(&self) -> Self::ShutdownFuture<'_> {
        // We could use shutdown op here, which requires kernel 5.11+.
        // However, for simplicity, we just close the socket using direct syscall.
        let fd = self.as_raw_fd();
        async move {
            match unsafe { libc::shutdown(fd, libc::SHUT_WR) } {
                -1 => Err(io::Error::last_os_error()),
                _ => Ok(()),
            }
        }
    }
}

impl AsyncReadRent for UnixStream {
    type ReadFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;
    type ReadvFuture<'a, B>
    where
        B: 'a,
    = impl std::future::Future<Output = crate::BufResult<usize, B>>;

    fn read<T: IoBufMut>(&self, buf: T) -> Self::ReadFuture<'_, T> {
        // Submit the read operation
        let op = Op::recv(&self.fd, buf).unwrap();
        op.read()
    }

    fn readv<T: IoVecBufMut>(&self, buf: T) -> Self::ReadvFuture<'_, T> {
        // Submit the read operation
        let op = Op::readv(&self.fd, buf).unwrap();
        op.read()
    }
}

impl std::fmt::Debug for UnixStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnixStream").field("fd", &self.fd).finish()
    }
}
