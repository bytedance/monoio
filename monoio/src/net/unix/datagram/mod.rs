//! Unix datagram related.

use std::{
    io,
    os::unix::{
        net::UnixDatagram as StdUnixDatagram,
        prelude::{AsRawFd, IntoRawFd, RawFd},
    },
    path::Path,
};

use super::{
    socket_addr::{local_addr, pair, peer_addr, socket_addr},
    SocketAddr,
};
use crate::driver::{op::Op, shared_fd::SharedFd};

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
        StdUnixDatagram::bind(path).and_then(Self::from_std)
    }

    /// Creates a new `UnixDatagram` which is not bound to any address.
    pub fn unbound() -> io::Result<Self> {
        StdUnixDatagram::unbound().and_then(Self::from_std)
    }

    /// Creates an unnamed pair of connected sockets.
    pub fn pair() -> io::Result<(Self, Self)> {
        let (a, b) = pair(libc::SOCK_DGRAM)?;
        Ok((Self::from_std(a)?, Self::from_std(b)?))
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
        completion.meta.result?;

        Ok(Self::from_shared_fd(completion.data.fd))
    }

    /// Creates new `UnixDatagram` from a `std::os::unix::net::UnixDatagram`.
    pub fn from_std(datagram: StdUnixDatagram) -> io::Result<Self> {
        let fd = datagram.into_raw_fd();
        Ok(Self::from_shared_fd(SharedFd::new(fd)?))
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

impl AsRawFd for UnixDatagram {
    #[inline]
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
