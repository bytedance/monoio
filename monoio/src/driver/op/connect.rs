use super::{super::shared_fd::SharedFd, Op, OpAble};

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
        let os_socket_addr = OsSocketAddr::from(socket_addr);
        let socket_type = socket_type | libc::SOCK_CLOEXEC;
        let domain = match socket_addr {
            SocketAddr::V4(_) => libc::AF_INET,
            SocketAddr::V6(_) => libc::AF_INET6,
        };
        let fd = crate::syscall!(socket(domain, socket_type, 0))?;

        Op::submit_with(Connect {
            fd: SharedFd::new(fd),
            os_socket_addr,
        })
    }
}

impl OpAble for Connect {
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::Connect::new(
            types::Fd(self.fd.raw_fd()),
            self.os_socket_addr.as_ptr(),
            self.os_socket_addr.len(),
        )
        .build()
    }
}

pub(crate) struct ConnectUnix {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    pub(crate) fd: SharedFd,
    socket_addr: libc::sockaddr_un,
    socket_len: libc::socklen_t,
}

impl Op<ConnectUnix> {
    /// Submit a request to connect.
    pub(crate) fn connect_unix(
        socket_addr: libc::sockaddr_un,
        socket_len: libc::socklen_t,
    ) -> io::Result<Op<ConnectUnix>> {
        let socket_type = libc::SOCK_STREAM | libc::SOCK_CLOEXEC;
        let domain = libc::AF_UNIX;
        let fd = crate::syscall!(socket(domain, socket_type, 0))?;

        Op::submit_with(ConnectUnix {
            fd: SharedFd::new(fd),
            socket_addr,
            socket_len,
        })
    }
}

impl OpAble for ConnectUnix {
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::Connect::new(
            types::Fd(self.fd.raw_fd()),
            &self.socket_addr as *const _ as *const _,
            self.socket_len,
        )
        .build()
    }
}
