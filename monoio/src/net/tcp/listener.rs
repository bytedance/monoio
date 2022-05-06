use std::cell::UnsafeCell;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::unix::prelude::{AsRawFd, FromRawFd, RawFd};
use std::{future::Future, io, net::ToSocketAddrs, os::unix::prelude::IntoRawFd};

use crate::{
    driver::{op::Op, shared_fd::SharedFd},
    net::ListenerConfig,
    stream::Stream,
};

use super::stream::TcpStream;

/// TcpListener
pub struct TcpListener {
    fd: SharedFd,
    sys_listener: Option<std::net::TcpListener>,
    meta: UnsafeCell<ListenerMeta>,
}

impl TcpListener {
    pub(crate) fn from_shared_fd(fd: SharedFd) -> Self {
        let sys_listener = unsafe { std::net::TcpListener::from_raw_fd(fd.raw_fd()) };
        Self {
            fd,
            sys_listener: Some(sys_listener),
            meta: UnsafeCell::new(ListenerMeta::default()),
        }
    }

    /// Bind to address with config
    pub fn bind_with_config<A: ToSocketAddrs>(
        addr: A,
        config: &ListenerConfig,
    ) -> io::Result<Self> {
        let addr = addr
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "empty address"))?;

        let domain = if addr.is_ipv6() {
            socket2::Domain::IPV6
        } else {
            socket2::Domain::IPV4
        };
        let sys_listener =
            socket2::Socket::new(domain, socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
        let addr = socket2::SockAddr::from(addr);

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

        let fd = SharedFd::new(sys_listener.into_raw_fd());

        Ok(Self::from_shared_fd(fd))
    }

    /// Bind to address
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let cfg = ListenerConfig::default();
        Self::bind_with_config(addr, &cfg)
    }

    /// Accept
    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let op = Op::accept(&self.fd)?;

        // Await the completion of the event
        let completion = op.await;

        // Convert fd
        let fd = completion.result?;

        // Construct stream
        let stream = TcpStream::from_shared_fd(SharedFd::new(fd as _));

        // Construct SocketAddr
        let storage = completion.data.addr.as_ptr() as *const _ as *const libc::sockaddr_storage;
        let addr = unsafe {
            match (*storage).ss_family as libc::c_int {
                libc::AF_INET => {
                    // Safety: if the ss_family field is AF_INET then storage must be a sockaddr_in.
                    let addr: &libc::sockaddr_in = &*(storage as *const libc::sockaddr_in);
                    let ip = Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes());
                    let port = u16::from_be(addr.sin_port);
                    SocketAddr::V4(SocketAddrV4::new(ip, port))
                }
                libc::AF_INET6 => {
                    // Safety: if the ss_family field is AF_INET6 then storage must be a sockaddr_in6.
                    let addr: &libc::sockaddr_in6 = &*(storage as *const libc::sockaddr_in6);
                    let ip = Ipv6Addr::from(addr.sin6_addr.s6_addr);
                    let port = u16::from_be(addr.sin6_port);
                    SocketAddr::V6(SocketAddrV6::new(
                        ip,
                        port,
                        addr.sin6_flowinfo,
                        addr.sin6_scope_id,
                    ))
                }
                _ => {
                    return Err(io::ErrorKind::InvalidInput.into());
                }
            }
        };

        Ok((stream, addr))
    }

    /// Returns the local address that this listener is bound to.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        let meta = self.meta.get();
        if let Some(addr) = unsafe { &*meta }.local_addr {
            return Ok(addr);
        }
        self.sys_listener
            .as_ref()
            .unwrap()
            .local_addr()
            .map(|addr| {
                unsafe { &mut *meta }.local_addr = Some(addr);
                addr
            })
    }
}

impl Stream for TcpListener {
    type Item = io::Result<(TcpStream, SocketAddr)>;

    type Future<'a> = impl Future<Output = Option<Self::Item>>;

    fn next(&mut self) -> Self::Future<'_> {
        async move { Some(self.accept().await) }
    }
}

impl std::fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpListener").field("fd", &self.fd).finish()
    }
}

impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl Drop for TcpListener {
    fn drop(&mut self) {
        self.sys_listener.take().unwrap().into_raw_fd();
    }
}

#[derive(Debug, Default, Clone)]
struct ListenerMeta {
    local_addr: Option<SocketAddr>,
}
