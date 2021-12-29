//! Unix datagram related.

use std::{
    io,
    os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
    path::Path,
};

use std::os::unix::net::UnixDatagram as StdUnixDatagram;

use crate::driver::{Op, SharedFd};

use super::{
    socket_addr::{local_addr, pair, peer_addr, socket_addr},
    SocketAddr,
};

/// UnixDatagram
pub struct UnixDatagram {
    fd: SharedFd,
}

impl UnixDatagram {
    pub(crate) fn from_shared_fd(fd: SharedFd) -> Self {
        Self { fd }
    }

    /// Creates a Unix datagram socket bound to the given path.
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        StdUnixDatagram::bind(path).map(Self::from_std)
    }

    /// Creates a new `UnixDatagram` which is not bound to any address.
    pub fn unbound() -> io::Result<Self> {
        StdUnixDatagram::unbound().map(Self::from_std)
    }

    /// Creates an unnamed pair of connected sockets.
    pub fn pair() -> io::Result<(Self, Self)> {
        pair(libc::SOCK_DGRAM).map(|(a, b)| (Self::from_std(a), Self::from_std(b)))
    }

    /// Connects the socket to the specified address.
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

        Ok(Self::from_shared_fd(completion.data.fd))
    }

    /// Creates new `UnixDatagram` from a `std::os::unix::net::UnixDatagram`.
    pub fn from_std(datagram: StdUnixDatagram) -> Self {
        let fd = datagram.into_raw_fd();
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

impl FromRawFd for UnixDatagram {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self::from_shared_fd(SharedFd::new(fd))
    }
}

impl AsRawFd for UnixDatagram {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl std::fmt::Debug for UnixDatagram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnixDatagram")
            .field("fd", &self.fd)
            .finish()
    }
}
