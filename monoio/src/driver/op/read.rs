use super::{super::shared_fd::SharedFd, Op};
use crate::{
    buf::{IoBufMut, IoVecBufMut},
    BufResult,
};

use std::io;

pub(crate) struct Read<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    /// Reference to the in-flight buffer.
    pub(crate) buf: T,
}

impl<T: IoBufMut> Op<Read<T>> {
    pub(crate) fn read_at(fd: &SharedFd, buf: T, offset: u64) -> io::Result<Op<Read<T>>> {
        use io_uring::{opcode, types};

        Op::submit_with(
            Read {
                fd: fd.clone(),
                buf,
            },
            |read| {
                opcode::Read::new(
                    types::Fd(fd.raw_fd()),
                    read.buf.write_ptr(),
                    read.buf.bytes_total() as _,
                )
                .offset(offset as _)
                .build()
            },
        )
    }

    pub(crate) async fn read(self) -> BufResult<usize, T> {
        let complete = self.await;

        // Convert the operation result to `usize`
        let res = complete.result.map(|v| v as usize);
        // Recover the buffer
        let mut buf = complete.data.buf;

        // If the operation was successful, advance the initialized cursor.
        if let Ok(n) = res {
            // Safety: the kernel wrote `n` bytes to the buffer.
            unsafe {
                buf.set_init(n);
            }
        }

        (res, buf)
    }
}

pub(crate) struct ReadVec<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    /// Reference to the in-flight buffer.
    pub(crate) buf_vec: T,
}

impl<T: IoVecBufMut> Op<ReadVec<T>> {
    pub(crate) fn readv(fd: &SharedFd, buf_vec: T) -> io::Result<Self> {
        use io_uring::{opcode, types};

        Op::submit_with(
            ReadVec {
                fd: fd.clone(),
                buf_vec,
            },
            |read_vec| {
                let ptr = read_vec.buf_vec.write_iovec_ptr() as _;
                let len = read_vec.buf_vec.write_iovec_len() as _;
                opcode::Readv::new(types::Fd(fd.raw_fd()), ptr, len).build()
            },
        )
    }

    pub(crate) async fn read(self) -> BufResult<usize, T> {
        let complete = self.await;
        let res = complete.result.map(|v| v as _);
        let mut buf_vec = complete.data.buf_vec;

        if let Ok(n) = res {
            // Safety: the kernel wrote `n` bytes to the buffer.
            unsafe {
                buf_vec.set_init(n);
            }
        }
        (res, buf_vec)
    }
}
