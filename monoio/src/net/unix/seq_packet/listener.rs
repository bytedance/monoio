use std::{
    future::Future,
    io,
    os::fd::{AsRawFd, RawFd},
    path::Path,
};

use super::UnixSeqpacket;
use crate::{
    driver::{op::Op, shared_fd::SharedFd},
    io::stream::Stream,
    net::{
        new_socket,
        unix::{socket_addr::socket_addr, SocketAddr},
    },
};

const DEFAULT_BACKLOG: libc::c_int = 128;

/// Listener for UnixSeqpacket
pub struct UnixSeqpacketListener {
    fd: SharedFd,
}

impl UnixSeqpacketListener {
    /// Creates a new `UnixSeqpacketListener` bound to the specified path with custom backlog
    pub fn bind_with_backlog<P: AsRef<Path>>(path: P, backlog: libc::c_int) -> io::Result<Self> {
        let (addr, addr_len) = socket_addr(path.as_ref())?;
        let socket = new_socket(libc::AF_UNIX, libc::SOCK_SEQPACKET)?;
        crate::syscall!(bind(socket, &addr as *const _ as *const _, addr_len))?;
        crate::syscall!(listen(socket, backlog))?;
        Ok(Self {
            fd: SharedFd::new(socket)?,
        })
    }

    /// Creates a new `UnixSeqpacketListener` bound to the specified path with default backlog(128)
    #[inline]
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Self::bind_with_backlog(path, DEFAULT_BACKLOG)
    }

    /// Accept a UnixSeqpacket
    pub async fn accept(&self) -> io::Result<(UnixSeqpacket, SocketAddr)> {
        let op = Op::accept(&self.fd)?;

        // Await the completion of the event
        let completion = op.await;

        // Convert fd
        let fd = completion.meta.result?;

        // Construct stream
        let stream = UnixSeqpacket::from_shared_fd(SharedFd::new(fd as _)?);

        // Construct SocketAddr
        let mut storage = unsafe { std::mem::MaybeUninit::assume_init(completion.data.addr.0) };
        let storage: *mut libc::sockaddr_storage = &mut storage as *mut _;
        let raw_addr_un: libc::sockaddr_un = unsafe { *storage.cast() };
        let raw_addr_len = completion.data.addr.1;

        let addr = SocketAddr::from_parts(raw_addr_un, raw_addr_len);

        Ok((stream, addr))
    }
}

impl AsRawFd for UnixSeqpacketListener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl std::fmt::Debug for UnixSeqpacketListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnixSeqpacketListener")
            .field("fd", &self.fd)
            .finish()
    }
}

impl Stream for UnixSeqpacketListener {
    type Item = io::Result<(UnixSeqpacket, SocketAddr)>;

    type NextFuture<'a> = impl Future<Output = Option<Self::Item>> + 'a;

    #[inline]
    fn next(&mut self) -> Self::NextFuture<'_> {
        async move { Some(self.accept().await) }
    }
}
