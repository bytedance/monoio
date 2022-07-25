use super::{super::shared_fd::SharedFd, Op, OpAble};

#[cfg(all(unix, feature = "legacy"))]
use crate::driver::legacy::ready::Direction;
#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};

use os_socketaddr::OsSocketAddr;
use std::{io, net::SocketAddr};

pub(crate) struct Connect {
    pub(crate) fd: SharedFd,
    os_socket_addr: OsSocketAddr,
}

impl Op<Connect> {
    /// Submit a request to connect.
    pub(crate) fn connect(
        socket_type: libc::c_int,
        socket_addr: SocketAddr,
    ) -> io::Result<Op<Connect>> {
        #[cfg(unix)]
        {
            let domain = match socket_addr {
                SocketAddr::V4(_) => libc::AF_INET,
                SocketAddr::V6(_) => libc::AF_INET6,
            };
            let socket = super::new_socket(domain, socket_type)?;
            let os_socket_addr = OsSocketAddr::from(socket_addr);

            Op::submit_with(Connect {
                fd: SharedFd::new(socket)?,
                os_socket_addr,
            })
        }
        #[cfg(windows)]
        unimplemented!()
    }
}

impl OpAble for Connect {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::Connect::new(
            types::Fd(self.fd.raw_fd()),
            self.os_socket_addr.as_ptr(),
            self.os_socket_addr.len(),
        )
        .build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(self: &mut std::pin::Pin<Box<Self>>) -> io::Result<u32> {
        match crate::syscall_u32!(connect(
            self.fd.raw_fd(),
            self.os_socket_addr.as_ptr(),
            self.os_socket_addr.len()
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
    socket_addr: libc::sockaddr_un,
    #[cfg(unix)]
    socket_len: libc::socklen_t,
}

impl Op<ConnectUnix> {
    #[cfg(unix)]

    /// Submit a request to connect.
    pub(crate) fn connect_unix(
        socket_addr: libc::sockaddr_un,
        socket_len: libc::socklen_t,
    ) -> io::Result<Op<ConnectUnix>> {
        let socket = super::new_socket(libc::AF_UNIX, libc::SOCK_STREAM)?;

        Op::submit_with(ConnectUnix {
            fd: SharedFd::new(socket)?,
            socket_addr,
            socket_len,
        })
    }
}

impl OpAble for ConnectUnix {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::Connect::new(
            types::Fd(self.fd.raw_fd()),
            &self.socket_addr as *const _ as *const _,
            self.socket_len,
        )
        .build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(self: &mut std::pin::Pin<Box<Self>>) -> io::Result<u32> {
        match crate::syscall_u32!(connect(
            self.fd.raw_fd(),
            &self.socket_addr as *const _ as *const _,
            self.socket_len
        )) {
            Err(err) if err.raw_os_error() != Some(libc::EINPROGRESS) => Err(err),
            _ => Ok(self.fd.raw_fd() as u32),
        }
    }
}
