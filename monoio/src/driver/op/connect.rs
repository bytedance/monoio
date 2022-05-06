use super::{super::shared_fd::SharedFd, Op};

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
        use io_uring::{opcode, types};

        let os_socket_addr = OsSocketAddr::from(socket_addr);
        let socket_type = socket_type | libc::SOCK_CLOEXEC;
        let domain = match socket_addr {
            SocketAddr::V4(_) => libc::AF_INET,
            SocketAddr::V6(_) => libc::AF_INET6,
        };
        let fd = syscall!(socket(domain, socket_type, 0))?;

        Op::submit_with(
            Connect {
                fd: SharedFd::new(fd),
                os_socket_addr,
            },
            |connect| {
                opcode::Connect::new(
                    types::Fd(fd),
                    connect.os_socket_addr.as_ptr(),
                    connect.os_socket_addr.len(),
                )
                .build()
            },
        )
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
        use io_uring::{opcode, types};

        let socket_type = libc::SOCK_STREAM | libc::SOCK_CLOEXEC;
        let domain = libc::AF_UNIX;
        let fd = syscall!(socket(domain, socket_type, 0))?;

        Op::submit_with(
            ConnectUnix {
                fd: SharedFd::new(fd),
                socket_addr,
                socket_len,
            },
            |connect| {
                opcode::Connect::new(
                    types::Fd(fd),
                    &connect.socket_addr as *const _ as *const _,
                    connect.socket_len,
                )
                .build()
            },
        )
    }
}
