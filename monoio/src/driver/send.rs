use crate::buf::IoBuf;
use crate::driver::{Op, SharedFd};
use crate::BufResult;

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
        // TODO: use libc const after supported.
        const MSG_ZEROCOPY: libc::c_int = 0x4000000;
        #[cfg(feature = "zero-copy")]
        const FLAGS: i32 = libc::MSG_NOSIGNAL | MSG_ZEROCOPY;
        #[cfg(not(feature = "zero-copy"))]
        const FLAGS: i32 = libc::MSG_NOSIGNAL;

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
                .flags(FLAGS)
                .build()
            },
        )
    }

    pub(crate) async fn write(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.result.map(|v| v as _), complete.data.buf)
    }
}
