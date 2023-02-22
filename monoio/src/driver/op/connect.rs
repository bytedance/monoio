use std::{io, net::SocketAddr};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(all(unix, feature = "legacy"))]
use crate::driver::legacy::ready::Direction;

pub(crate) struct Connect {
    pub(crate) fd: SharedFd,
    socket_addr: Box<SocketAddrCRepr>,
    socket_addr_len: libc::socklen_t,
}

impl Op<Connect> {
    /// Submit a request to connect.
    pub(crate) fn connect(socket: SharedFd, addr: SocketAddr) -> io::Result<Op<Connect>> {
        #[cfg(unix)]
        {
            let (raw_addr, raw_addr_length) = socket_addr(&addr);
            Op::submit_with(Connect {
                fd: socket,
                socket_addr: Box::new(raw_addr),
                socket_addr_len: raw_addr_length,
            })
        }
        #[cfg(windows)]
        unimplemented!()
    }
}

impl OpAble for Connect {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::Connect::new(
            types::Fd(self.fd.raw_fd()),
            self.socket_addr.as_ptr(),
            self.socket_addr_len,
        )
        .build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        match crate::syscall_u32!(connect(
            self.fd.raw_fd(),
            self.socket_addr.as_ptr(),
            self.socket_addr_len,
        )) {
            Err(err) if err.raw_os_error() != Some(libc::EINPROGRESS) => Err(err),
            _ => Ok(self.fd.raw_fd() as u32),
        }
    }
}

pub(crate) struct ConnectUnix {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    pub(crate) fd: SharedFd,
    #[cfg(unix)]
    socket_addr: Box<(libc::sockaddr_un, libc::socklen_t)>,
}

impl Op<ConnectUnix> {
    #[cfg(unix)]

    /// Submit a request to connect.
    pub(crate) fn connect_unix(
        socket: SharedFd,
        socket_addr: libc::sockaddr_un,
        socket_len: libc::socklen_t,
    ) -> io::Result<Op<ConnectUnix>> {
        Op::submit_with(ConnectUnix {
            fd: socket,
            socket_addr: Box::new((socket_addr, socket_len)),
        })
    }
}

impl OpAble for ConnectUnix {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::Connect::new(
            types::Fd(self.fd.raw_fd()),
            &self.socket_addr.0 as *const _ as *const _,
            self.socket_addr.1,
        )
        .build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        match crate::syscall_u32!(connect(
            self.fd.raw_fd(),
            &self.socket_addr.0 as *const _ as *const _,
            self.socket_addr.1
        )) {
            Err(err) if err.raw_os_error() != Some(libc::EINPROGRESS) => Err(err),
            _ => Ok(self.fd.raw_fd() as u32),
        }
    }
}

/// A type with the same memory layout as `libc::sockaddr`. Used in converting Rust level
/// SocketAddr* types into their system representation. The benefit of this specific
/// type over using `libc::sockaddr_storage` is that this type is exactly as large as it
/// needs to be and not a lot larger. And it can be initialized cleaner from Rust.
// Copied from mio.
#[repr(C)]
pub(crate) union SocketAddrCRepr {
    v4: libc::sockaddr_in,
    v6: libc::sockaddr_in6,
}

impl SocketAddrCRepr {
    pub(crate) fn as_ptr(&self) -> *const libc::sockaddr {
        self as *const _ as *const libc::sockaddr
    }
}

/// Converts a Rust `SocketAddr` into the system representation.
pub(crate) fn socket_addr(addr: &SocketAddr) -> (SocketAddrCRepr, libc::socklen_t) {
    match addr {
        SocketAddr::V4(ref addr) => {
            // `s_addr` is stored as BE on all machine and the array is in BE order.
            // So the native endian conversion method is used so that it's never swapped.
            let sin_addr = libc::in_addr {
                s_addr: u32::from_ne_bytes(addr.ip().octets()),
            };

            let sockaddr_in = libc::sockaddr_in {
                sin_family: libc::AF_INET as libc::sa_family_t,
                sin_port: addr.port().to_be(),
                sin_addr,
                sin_zero: [0; 8],
                #[cfg(any(
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "ios",
                    target_os = "macos",
                    target_os = "netbsd",
                    target_os = "openbsd"
                ))]
                sin_len: 0,
            };

            let sockaddr = SocketAddrCRepr { v4: sockaddr_in };
            let socklen = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;
            (sockaddr, socklen)
        }
        SocketAddr::V6(ref addr) => {
            let sockaddr_in6 = libc::sockaddr_in6 {
                sin6_family: libc::AF_INET6 as libc::sa_family_t,
                sin6_port: addr.port().to_be(),
                sin6_addr: libc::in6_addr {
                    s6_addr: addr.ip().octets(),
                },
                sin6_flowinfo: addr.flowinfo(),
                sin6_scope_id: addr.scope_id(),
                #[cfg(any(
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "ios",
                    target_os = "macos",
                    target_os = "netbsd",
                    target_os = "openbsd"
                ))]
                sin6_len: 0,
                #[cfg(target_os = "illumos")]
                __sin6_src_id: 0,
            };

            let sockaddr = SocketAddrCRepr { v6: sockaddr_in6 };
            let socklen = std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t;
            (sockaddr, socklen)
        }
    }
}
