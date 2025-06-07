use std::{
    future::Future,
    io,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::prelude::RawFd,
    },
    process::Stdio,
};

use crate::{
    buf::{IoBufMut, IoVecBufMut},
    driver::{op::Op, shared_fd::SharedFd},
    io::{
        as_fd::{AsReadFd, AsWriteFd, SharedFdWrapper},
        AsyncReadRent,
    },
    BufResult,
};

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
    crate::syscall!(pipe2@RAW(pipes.as_mut_ptr() as _, flag))?;
    #[cfg(not(target_os = "linux"))]
    crate::syscall!(pipe@RAW(pipes.as_mut_ptr() as _))?;
    Ok((Pipe::from_raw_fd(pipes[0]), Pipe::from_raw_fd(pipes[1])))
}

impl AsReadFd for Pipe {
    #[inline]
    fn as_reader_fd(&mut self) -> &SharedFdWrapper {
        SharedFdWrapper::new(&self.fd)
    }
}

impl AsWriteFd for Pipe {
    #[inline]
    fn as_writer_fd(&mut self) -> &SharedFdWrapper {
        SharedFdWrapper::new(&self.fd)
    }
}

impl IntoRawFd for Pipe {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.fd
            .try_unwrap()
            .expect("unexpected multiple reference to rawfd")
    }
}

impl AsRawFd for Pipe {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.raw_fd()
    }
}

impl From<Pipe> for Stdio {
    #[inline]
    fn from(pipe: Pipe) -> Self {
        let rawfd = pipe.fd.try_unwrap().unwrap();
        unsafe { Stdio::from_raw_fd(rawfd) }
    }
}

impl AsyncReadRent for Pipe {
    #[inline]
    fn read<T: IoBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        // Submit the read operation
        let op = Op::read(self.fd.clone(), buf).unwrap();
        op.result()
    }

    #[inline]
    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        // Submit the read operation
        let op = Op::readv(self.fd.clone(), buf).unwrap();
        op.result()
    }
}
