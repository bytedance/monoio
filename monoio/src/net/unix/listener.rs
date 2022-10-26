use std::{
    future::Future,
    io,
    os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
    path::Path,
};

use super::{socket_addr::SocketAddr, UnixStream};
use crate::{
    driver::{op::Op, shared_fd::SharedFd},
    io::stream::Stream,
    net::ListenerConfig,
};

/// UnixListener
pub struct UnixListener {
    fd: SharedFd,
    sys_listener: Option<std::os::unix::net::UnixListener>,
}

impl UnixListener {
    pub(crate) fn from_shared_fd(fd: SharedFd) -> Self {
        let sys_listener = unsafe { std::os::unix::net::UnixListener::from_raw_fd(fd.raw_fd()) };
        Self {
            fd,
            sys_listener: Some(sys_listener),
        }
    }

    /// Creates a new `UnixListener` bound to the specified socket with custom
    /// config.
    pub fn bind_with_config<P: AsRef<Path>>(
        path: P,
        config: &ListenerConfig,
    ) -> io::Result<UnixListener> {
        let sys_listener =
            socket2::Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?;
        let addr = socket2::SockAddr::unix(path)?;

        if config.reuse_port {
            sys_listener.set_reuse_port(true)?;
        }
        if config.reuse_addr {
            sys_listener.set_reuse_address(true)?;
        }
        if let Some(send_buf_size) = config.send_buf_size {
            sys_listener.set_send_buffer_size(send_buf_size)?;
        }
        if let Some(recv_buf_size) = config.recv_buf_size {
            sys_listener.set_recv_buffer_size(recv_buf_size)?;
        }

        sys_listener.bind(&addr)?;
        sys_listener.listen(config.backlog)?;

        let fd = SharedFd::new(sys_listener.into_raw_fd())?;

        Ok(Self::from_shared_fd(fd))
    }

    /// Creates a new `UnixListener` bound to the specified socket with default
    /// config.
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        Self::bind_with_config(path, &ListenerConfig::default())
    }

    /// Accept
    pub async fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let op = Op::accept(&self.fd)?;

        // Await the completion of the event
        let completion = op.await;

        // Convert fd
        let fd = completion.meta.result?;

        // Construct stream
        let stream = UnixStream::from_shared_fd(SharedFd::new(fd as _)?);

        // Construct SocketAddr
        let mut storage = unsafe { std::mem::MaybeUninit::assume_init(completion.data.addr.0) };
        let storage: *mut libc::sockaddr_storage = &mut storage as *mut _;
        let raw_addr_un: libc::sockaddr_un = unsafe { *storage.cast() };
        let raw_addr_len = completion.data.addr.1;

        let addr = SocketAddr::from_parts(raw_addr_un, raw_addr_len);

        Ok((stream, addr))
    }
}

impl Stream for UnixListener {
    type Item = io::Result<(UnixStream, SocketAddr)>;

    type NextFuture<'a> = impl Future<Output = Option<Self::Item>> + 'a;

    #[inline]
    fn next(&mut self) -> Self::NextFuture<'_> {
        async move { Some(self.accept().await) }
    }
}

impl std::fmt::Debug for UnixListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnixListener")
            .field("fd", &self.fd)
            .finish()
    }
}

impl AsRawFd for UnixListener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl Drop for UnixListener {
    #[inline]
    fn drop(&mut self) {
        self.sys_listener.take().unwrap().into_raw_fd();
    }
}
