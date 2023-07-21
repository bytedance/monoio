//! Network related
//! Currently, TCP/UnixStream/UnixDatagram are implemented.

mod listener_config;
pub mod tcp;
pub mod udp;
#[cfg(unix)]
pub mod unix;

pub use listener_config::ListenerOpts;
#[deprecated(since = "0.2.0", note = "use ListenerOpts")]
pub use listener_config::ListenerOpts as ListenerConfig;
pub use tcp::{TcpConnectOpts, TcpListener, TcpStream};
#[cfg(unix)]
pub use unix::{Pipe, UnixDatagram, UnixListener, UnixStream};

// Copied from mio.
pub(crate) fn new_socket(
    domain: libc::c_int,
    socket_type: libc::c_int,
) -> std::io::Result<libc::c_int> {
    #[cfg(any(
        target_os = "android",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "illumos",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    let socket_type = socket_type | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC;

    #[cfg(target_os = "linux")]
    let socket_type = {
        if crate::driver::op::is_legacy() {
            socket_type | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK
        } else {
            socket_type | libc::SOCK_CLOEXEC
        }
    };

    // Gives a warning for platforms without SOCK_NONBLOCK.
    #[allow(clippy::let_and_return)]
    #[cfg(unix)]
    let socket = crate::syscall!(socket(domain, socket_type, 0));

    // Mimic `libstd` and set `SO_NOSIGPIPE` on apple systems.
    #[cfg(target_vendor = "apple")]
    let socket = socket.and_then(|socket| {
        crate::syscall!(setsockopt(
            socket,
            libc::SOL_SOCKET,
            libc::SO_NOSIGPIPE,
            &1 as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t
        ))
        .map(|_| socket)
    });

    // Darwin doesn't have SOCK_NONBLOCK or SOCK_CLOEXEC.
    #[cfg(any(target_os = "ios", target_os = "macos"))]
    let socket = socket.and_then(|socket| {
        // For platforms that don't support flags in socket, we need to
        // set the flags ourselves.
        crate::syscall!(fcntl(socket, libc::F_SETFL, libc::O_NONBLOCK))
            .and_then(|_| {
                crate::syscall!(fcntl(socket, libc::F_SETFD, libc::FD_CLOEXEC)).map(|_| socket)
            })
            .map_err(|e| {
                // If either of the `fcntl` calls failed, ensure the socket is
                // closed and return the error.
                let _ = crate::syscall!(close(socket));
                e
            })
    });

    #[cfg(windows)]
    let socket: std::io::Result<_> = unimplemented!();

    socket
}
