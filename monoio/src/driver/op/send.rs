use super::{super::shared_fd::SharedFd, Op};
use crate::{buf::IoBuf, BufResult};

use std::io;

pub(crate) struct Send<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    pub(crate) buf: T,
}

impl<T: IoBuf> Op<Send<T>> {
    pub(crate) fn send(fd: &SharedFd, buf: T) -> io::Result<Self> {
        use io_uring::{opcode, types};

        #[cfg(feature = "zero-copy")]
        fn zero_copy_flag_guard<T: IoBuf>(buf: &T) -> i32 {
            // TODO: use libc const after supported.
            const MSG_ZEROCOPY: libc::c_int = 0x4000000;
            // According to Linux's documentation, zero copy introduces extra overhead and
            // is only considered effective for at writes over around 10 KB.
            // see also: https://www.kernel.org/doc/html/v4.16/networking/msg_zerocopy.html
            const MSG_ZEROCOPY_THRESHOLD: usize = 10 * 1024 * 1024;
            if buf.bytes_init() >= MSG_ZEROCOPY_THRESHOLD {
                libc::MSG_NOSIGNAL | MSG_ZEROCOPY
            } else {
                libc::MSG_NOSIGNAL
            }
        }

        #[cfg(feature = "zero-copy")]
        let flags = zero_copy_flag_guard(&buf);
        #[cfg(not(feature = "zero-copy"))]
        let flags = libc::MSG_NOSIGNAL;

        Op::submit_with(
            Send {
                fd: fd.clone(),
                buf,
            },
            |send| {
                opcode::Send::new(
                    types::Fd(fd.raw_fd()),
                    send.buf.read_ptr(),
                    send.buf.bytes_init() as _,
                )
                .flags(flags)
                .build()
            },
        )
    }

    pub(crate) async fn write(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.result.map(|v| v as _), complete.data.buf)
    }
}
