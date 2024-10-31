use std::{io, os::fd::AsRawFd};

/// Call before listen.
pub(crate) fn set_tcp_fastopen<S: AsRawFd>(fd: &S) -> io::Result<()> {
    const ENABLED: libc::c_int = 0x1;
    crate::syscall!(setsockopt@RAW(
        fd.as_raw_fd(),
        libc::IPPROTO_TCP,
        libc::TCP_FASTOPEN,
        &ENABLED as *const _ as *const libc::c_void,
        std::mem::size_of::<libc::c_int>() as libc::socklen_t
    ))?;
    Ok(())
}

/// Force use fastopen.
/// MacOS only.
pub(crate) fn set_tcp_fastopen_force_enable<S: AsRawFd>(fd: &S) -> io::Result<()> {
    const TCP_FASTOPEN_FORCE_ENABLE: libc::c_int = 0x218;
    const ENABLED: libc::c_int = 0x1;

    crate::syscall!(setsockopt@RAW(
        fd.as_raw_fd(),
        libc::IPPROTO_TCP,
        TCP_FASTOPEN_FORCE_ENABLE,
        &ENABLED as *const _ as *const libc::c_void,
        std::mem::size_of::<libc::c_int>() as libc::socklen_t
    ))?;
    Ok(())
}
