use std::{io, os::unix::prelude::RawFd};

use crate::driver::shared_fd::SharedFd;

/// Unix pipe.
pub struct Pipe {
    #[allow(dead_code)]
    pub(crate) fd: SharedFd,
}

impl Pipe {
    pub(crate) fn from_shared_fd(fd: SharedFd) -> Self {
        Self { fd }
    }

    fn from_raw_fd(fd: RawFd) -> Self {
        Self::from_shared_fd(SharedFd::new_without_register(fd))
    }
}

/// Create a new pair of pipe.
pub fn new_pipe() -> io::Result<(Pipe, Pipe)> {
    let mut pipes = [0 as libc::c_int; 2];
    #[cfg(target_os = "linux")]
    let flag = {
        if crate::driver::op::is_legacy() {
            libc::O_NONBLOCK
        } else {
            0
        }
    };
    #[cfg(target_os = "linux")]
    crate::syscall!(pipe2(pipes.as_mut_ptr() as _, flag))?;
    #[cfg(not(target_os = "linux"))]
    crate::syscall!(pipe(pipes.as_mut_ptr() as _))?;
    Ok((Pipe::from_raw_fd(pipes[0]), Pipe::from_raw_fd(pipes[1])))
}
