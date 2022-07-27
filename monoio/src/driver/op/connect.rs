use std::{io, net::SocketAddr};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
use os_socketaddr::OsSocketAddr;

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(all(unix, feature = "legacy"))]
use crate::driver::legacy::ready::Direction;

pub(crate) struct Connect {
    pub(crate) fd: SharedFd,
    os_socket_addr: Box<OsSocketAddr>,
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
            let os_socket_addr = Box::new(OsSocketAddr::from(socket_addr));

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
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
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
    fn legacy_call(&mut self) -> io::Result<u32> {
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
    socket_addr: Box<(libc::sockaddr_un, libc::socklen_t)>,
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
