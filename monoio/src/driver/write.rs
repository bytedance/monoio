use crate::buf::{IoBuf, IoVecBuf};
use crate::driver::{Op, SharedFd};
use crate::BufResult;

use std::io;

pub(crate) struct Write<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    pub(crate) buf: T,
}

impl<T: IoBuf> Op<Write<T>> {
    pub(crate) fn write_at(fd: &SharedFd, buf: T, offset: u64) -> io::Result<Op<Write<T>>> {
        use io_uring::{opcode, types};

        Op::submit_with(
            Write {
                fd: fd.clone(),
                buf,
            },
            |write| {
                opcode::Write::new(
                    types::Fd(fd.raw_fd()),
                    write.buf.read_ptr(),
                    write.buf.bytes_init() as _,
                )
                .offset(offset as _)
                .build()
            },
        )
    }

    pub(crate) async fn write(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.result.map(|v| v as _), complete.data.buf)
    }
}

pub(crate) struct WriteVec<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    pub(crate) buf_vec: T,
}

impl<T: IoVecBuf> Op<WriteVec<T>> {
    pub(crate) fn writev(fd: &SharedFd, buf_vec: T) -> io::Result<Self> {
        use io_uring::{opcode, types};

        Op::submit_with(
            WriteVec {
                fd: fd.clone(),
                buf_vec,
            },
            |writev| {
                opcode::Writev::new(
                    types::Fd(fd.raw_fd()),
                    writev.buf_vec.stable_iovec_ptr(),
                    writev.buf_vec.iovec_len() as _,
                )
                .build()
            },
        )
    }

    pub(crate) async fn write(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.result.map(|v| v as _), complete.data.buf_vec)
    }
}
