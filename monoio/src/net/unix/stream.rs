use super::{
    socket_addr::{local_addr, pair, peer_addr, socket_addr, SocketAddr},
    split::{split, split_owned, OwnedReadHalf, OwnedWriteHalf, ReadHalf, WriteHalf},
    ucred::UCred,
};
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    driver::{op::Op, shared_fd::SharedFd},
    io::{AsyncReadRent, AsyncWriteRent},
};
use std::{
    io,
    os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
    path::Path,
};

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
        Self::inner_connect(addr, addr_len).await
    }

    /// Connects the socket to an address.
    pub async fn connect_addr(addr: SocketAddr) -> io::Result<Self> {
        let (addr, addr_len) = addr.into_parts();
        Self::inner_connect(addr, addr_len).await
    }

    #[inline(always)]
    async fn inner_connect(
        sockaddr: libc::sockaddr_un,
        socklen: libc::socklen_t,
    ) -> io::Result<Self> {
        let op = Op::connect_unix(sockaddr, socklen)?;
        let completion = op.await;
        completion.result?;

        let stream = Self::from_shared_fd(completion.data.fd);
        Ok(stream)
    }

    /// Creates an unnamed pair of connected sockets.
    ///
    /// Returns two `UnixStream`s which are connected to each other.
    pub fn pair() -> io::Result<(Self, Self)> {
        pair(libc::SOCK_STREAM).map(|(a, b)| (Self::from_std(a), Self::from_std(b)))
    }

    /// Returns effective credentials of the process which called `connect` or `pair`.
    pub fn peer_cred(&self) -> io::Result<UCred> {
        super::ucred::get_peer_cred(self)
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

    /// Split stream into read and write halves.
    #[allow(clippy::needless_lifetimes)]
    pub fn split<'a>(&'a mut self) -> (ReadHalf<'a>, WriteHalf<'a>) {
        split(self)
    }

    /// Split stream into read and write halves with ownership.
    pub fn into_split(self) -> (OwnedReadHalf, OwnedWriteHalf) {
        split_owned(self)
    }
}

impl FromRawFd for UnixStream {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self::from_shared_fd(SharedFd::new(fd))
    }
}

impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl std::fmt::Debug for UnixStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnixStream").field("fd", &self.fd).finish()
    }
}

impl AsyncWriteRent for UnixStream {
    type WriteFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;
    type WritevFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;
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
    type ReadFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;
    type ReadvFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: 'a;

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
