use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(all(unix, feature = "legacy"))]
use {
    crate::{driver::legacy::ready::Direction, syscall_u32},
    std::os::unix::prelude::AsRawFd,
};

use super::{super::shared_fd::SharedFd, Op, OpAble};
use crate::{buf::IoBuf, BufResult};

pub(crate) struct Send<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    pub(crate) buf: T,
}

impl<T: IoBuf> Op<Send<T>> {
    pub(crate) fn send(fd: &SharedFd, buf: T) -> io::Result<Self> {
        Op::submit_with(Send {
            fd: fd.clone(),
            buf,
        })
    }

    #[allow(unused)]
    pub(crate) fn send_raw(fd: &SharedFd, buf: T) -> Send<T> {
        Send {
            fd: fd.clone(),
            buf,
        }
    }

    pub(crate) async fn write(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.meta.result.map(|v| v as _), complete.data.buf)
    }
}

impl<T: IoBuf> OpAble for Send<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        #[cfg(feature = "zero-copy")]
        fn zero_copy_flag_guard<T: IoBuf>(buf: &T) -> libc::c_int {
            // TODO: use libc const after supported.
            const MSG_ZEROCOPY: libc::c_int = 0x4000000;
            // According to Linux's documentation, zero copy introduces extra overhead and
            // is only considered effective for at writes over around 10 KB.
            // see also: https://www.kernel.org/doc/html/v4.16/networking/msg_zerocopy.html
            const MSG_ZEROCOPY_THRESHOLD: usize = 10 * 1024 * 1024;
            if buf.bytes_init() >= MSG_ZEROCOPY_THRESHOLD {
                #[allow(deprecated)]
                libc::MSG_NOSIGNAL as libc::c_int
                    | MSG_ZEROCOPY
            } else {
                #[allow(deprecated)]
                libc::MSG_NOSIGNAL as libc::c_int
            }
        }

        #[cfg(feature = "zero-copy")]
        let flags = zero_copy_flag_guard(&self.buf);
        #[cfg(not(feature = "zero-copy"))]
        #[allow(deprecated)]
        let flags = libc::MSG_NOSIGNAL as libc::c_int;

        opcode::Send::new(
            types::Fd(self.fd.raw_fd()),
            self.buf.read_ptr(),
            self.buf.bytes_init() as _,
        )
        .flags(flags)
        .build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd
            .registered_index()
            .map(|idx| (Direction::Write, idx))
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_fd();
        #[cfg(target_os = "linux")]
        #[allow(deprecated)]
        let flags = libc::MSG_NOSIGNAL as _;
        #[cfg(not(target_os = "linux"))]
        let flags = 0;

        if self.buf.bytes_init() == 0 {
            return Ok(0);
        }

        syscall_u32!(send(
            fd,
            self.buf.read_ptr() as _,
            self.buf.bytes_init(),
            flags
        ))
    }
}
