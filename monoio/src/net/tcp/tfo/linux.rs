use std::{cell::Cell, io, os::fd::AsRawFd};

#[thread_local]
pub(crate) static TFO_CONNECT_AVAILABLE: Cell<bool> = Cell::new(true);

/// Call before listen.
pub(crate) fn set_tcp_fastopen<S: AsRawFd>(fd: &S, fast_open: i32) -> io::Result<()> {
    crate::syscall!(setsockopt(
        fd.as_raw_fd(),
        libc::SOL_TCP,
        libc::TCP_FASTOPEN,
        &fast_open as *const _ as *const libc::c_void,
        std::mem::size_of::<libc::c_int>() as libc::socklen_t
    ))?;
    Ok(())
}

/// Call before connect.
/// Linux 4.1+ only.
pub(crate) fn set_tcp_fastopen_connect<S: AsRawFd>(fd: &S) -> io::Result<()> {
    const ENABLED: libc::c_int = 0x1;

    crate::syscall!(setsockopt(
        fd.as_raw_fd(),
        libc::SOL_TCP,
        libc::TCP_FASTOPEN_CONNECT,
        &ENABLED as *const _ as *const libc::c_void,
        std::mem::size_of::<libc::c_int>() as libc::socklen_t
    ))?;
    Ok(())
}

pub(crate) fn try_set_tcp_fastopen_connect<S: AsRawFd>(fd: &S) {
    if !TFO_CONNECT_AVAILABLE.get() {
        return;
    }
    match set_tcp_fastopen_connect(fd) {
        Ok(_) => (),
        Err(e) if e.raw_os_error() == Some(libc::ENOPROTOOPT) => {
            TFO_CONNECT_AVAILABLE.set(false);
        }
        Err(_e) => {
            #[cfg(all(debug_assertions, feature = "debug"))]
            tracing::warn!("set_tcp_fastopen_connect failed: {}", _e);
        }
    }
}
