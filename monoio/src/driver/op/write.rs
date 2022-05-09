use super::{super::shared_fd::SharedFd, Op, OpAble};
use crate::{
    buf::{IoBuf, IoVecBuf},
    BufResult,
};

use io_uring::{opcode, types};
use std::io;

pub(crate) struct Write<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,
    offset: libc::off_t,

    pub(crate) buf: T,
}

impl<T: IoBuf> Op<Write<T>> {
    pub(crate) fn write_at(fd: &SharedFd, buf: T, offset: u64) -> io::Result<Op<Write<T>>> {
        Op::submit_with(Write {
            fd: fd.clone(),
            offset: offset as _,
            buf,
        })
    }

    pub(crate) async fn write(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.result.map(|v| v as _), complete.data.buf)
    }
}

impl<T: IoBuf> OpAble for Write<T> {
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::Write::new(
            types::Fd(self.fd.raw_fd()),
            self.buf.read_ptr(),
            self.buf.bytes_init() as _,
        )
        .offset(self.offset)
        .build()
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
        Op::submit_with(WriteVec {
            fd: fd.clone(),
            buf_vec,
        })
    }

    pub(crate) async fn write(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.result.map(|v| v as _), complete.data.buf_vec)
    }
}

impl<T: IoVecBuf> OpAble for WriteVec<T> {
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        let ptr = self.buf_vec.read_iovec_ptr() as *const _;
        let len = self.buf_vec.read_iovec_len() as _;
        opcode::Writev::new(types::Fd(self.fd.raw_fd()), ptr, len).build()
    }
}
