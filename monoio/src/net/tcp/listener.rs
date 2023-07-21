#[cfg(unix)]
use std::os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
#[cfg(windows)]
use std::os::windows::prelude::{AsRawHandle, FromRawSocket, RawHandle};
use std::{
    cell::UnsafeCell,
    future::Future,
    io,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
};

use super::stream::TcpStream;
#[cfg(unix)]
use crate::io::CancelHandle;
use crate::{
    driver::{op::Op, shared_fd::SharedFd},
    io::stream::Stream,
    net::ListenerOpts,
};

/// TcpListener
pub struct TcpListener {
    fd: SharedFd,
    sys_listener: Option<std::net::TcpListener>,
    meta: UnsafeCell<ListenerMeta>,
}

impl TcpListener {
    pub(crate) fn from_shared_fd(fd: SharedFd) -> Self {
        #[cfg(unix)]
        let sys_listener = unsafe { std::net::TcpListener::from_raw_fd(fd.raw_fd()) };
        #[cfg(windows)]
        let sys_listener = unsafe { std::net::TcpListener::from_raw_socket(todo!()) };
        Self {
            fd,
            sys_listener: Some(sys_listener),
            meta: UnsafeCell::new(ListenerMeta::default()),
        }
    }

    /// Bind to address with config
    pub fn bind_with_config<A: ToSocketAddrs>(addr: A, opts: &ListenerOpts) -> io::Result<Self> {
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

        #[cfg(all(unix, feature = "legacy"))]
        Self::set_non_blocking(&sys_listener)?;

        let addr = socket2::SockAddr::from(addr);
        #[cfg(unix)]
        if opts.reuse_port {
            sys_listener.set_reuse_port(true)?;
        }
        if opts.reuse_addr {
            sys_listener.set_reuse_address(true)?;
        }
        if let Some(send_buf_size) = opts.send_buf_size {
            sys_listener.set_send_buffer_size(send_buf_size)?;
        }
        if let Some(recv_buf_size) = opts.recv_buf_size {
            sys_listener.set_recv_buffer_size(recv_buf_size)?;
        }
        if opts.tcp_fast_open {
            #[cfg(any(target_os = "linux", target_os = "android"))]
            super::tfo::set_tcp_fastopen(&sys_listener, opts.backlog)?;
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            let _ = super::tfo::set_tcp_fastopen_force_enable(&sys_listener);
        }
        sys_listener.bind(&addr)?;
        sys_listener.listen(opts.backlog)?;

        #[cfg(any(target_os = "ios", target_os = "macos"))]
        if opts.tcp_fast_open {
            super::tfo::set_tcp_fastopen(&sys_listener)?;
        }

        #[cfg(unix)]
        let fd = SharedFd::new(sys_listener.into_raw_fd())?;

        #[cfg(windows)]
        let fd = unimplemented!();

        Ok(Self::from_shared_fd(fd))
    }

    /// Bind to address
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        const DEFAULT_CFG: ListenerOpts = ListenerOpts::new();
        Self::bind_with_config(addr, &DEFAULT_CFG)
    }

    #[cfg(unix)]
    /// Accept
    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let op = Op::accept(&self.fd)?;

        // Await the completion of the event
        let completion = op.await;

        // Convert fd
        let fd = completion.meta.result?;

        // Construct stream
        let stream = TcpStream::from_shared_fd(SharedFd::new(fd as _)?);

        // Construct SocketAddr
        let storage = completion.data.addr.0.as_ptr();
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
                    // Safety: if the ss_family field is AF_INET6 then storage must be a
                    // sockaddr_in6.
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

    #[cfg(unix)]
    /// Cancelable accept
    pub async fn cancelable_accept(&self, c: CancelHandle) -> io::Result<(TcpStream, SocketAddr)> {
        use crate::io::operation_canceled;

        if c.canceled() {
            return Err(operation_canceled());
        }
        let op = Op::accept(&self.fd)?;
        let _guard = c.associate_op(op.op_canceller());

        // Await the completion of the event
        let completion = op.await;

        // Convert fd
        let fd = completion.meta.result?;

        // Construct stream
        let stream = TcpStream::from_shared_fd(SharedFd::new(fd as _)?);

        // Construct SocketAddr
        let storage = completion.data.addr.0.as_ptr();
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
                    // Safety: if the ss_family field is AF_INET6 then storage must be a
                    // sockaddr_in6.
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

    #[cfg(windows)]
    /// Accept
    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        unimplemented!()
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

    #[cfg(all(unix, feature = "legacy"))]
    fn set_non_blocking(_socket: &socket2::Socket) -> io::Result<()> {
        crate::driver::CURRENT.with(|x| match x {
            // TODO: windows ioring support
            #[cfg(all(target_os = "linux", feature = "iouring"))]
            crate::driver::Inner::Uring(_) => Ok(()),
            crate::driver::Inner::Legacy(_) => _socket.set_nonblocking(true),
        })
    }

    /// Wait for read readiness.
    /// Note: Do not use it before every io. It is different from other runtimes!
    ///
    /// Everytime call to this method may pay a syscall cost.
    /// In uring impl, it will push a PollAdd op; in epoll impl, it will use use
    /// inner readiness state; if !relaxed, it will call syscall poll after that.
    ///
    /// If relaxed, on legacy driver it may return false positive result.
    /// If you want to do io by your own, you must maintain io readiness and wait
    /// for io ready with relaxed=false.
    pub async fn readable(&self, relaxed: bool) -> io::Result<()> {
        let op = Op::poll_read(&self.fd, relaxed).unwrap();
        op.wait().await
    }

    /// Creates new `TcpListener` from a `std::net::TcpListener`.
    pub fn from_std(stdl: std::net::TcpListener) -> io::Result<Self> {
        match SharedFd::new(stdl.as_raw_fd()) {
            Ok(shared) => {
                stdl.into_raw_fd();
                Ok(Self::from_shared_fd(shared))
            }
            Err(e) => Err(e),
        }
    }
}

impl Stream for TcpListener {
    type Item = io::Result<(TcpStream, SocketAddr)>;

    type NextFuture<'a> = impl Future<Output = Option<Self::Item>> + 'a;

    #[inline]
    fn next(&mut self) -> Self::NextFuture<'_> {
        async move { Some(self.accept().await) }
    }
}

impl std::fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpListener").field("fd", &self.fd).finish()
    }
}

#[cfg(unix)]
impl AsRawFd for TcpListener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

#[cfg(windows)]
impl AsRawHandle for TcpListener {
    fn as_raw_handle(&self) -> RawHandle {
        self.fd.raw_handle()
    }
}

impl Drop for TcpListener {
    #[inline]
    fn drop(&mut self) {
        #[cfg(unix)]
        self.sys_listener.take().unwrap().into_raw_fd();
        #[cfg(windows)]
        unimplemented!()
    }
}

#[derive(Debug, Default, Clone)]
struct ListenerMeta {
    local_addr: Option<SocketAddr>,
}
