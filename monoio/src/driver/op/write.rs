use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(all(unix, feature = "legacy"))]
use {
    crate::{driver::legacy::ready::Direction, syscall_u32},
    std::os::unix::prelude::AsRawFd,
};

use super::{super::shared_fd::SharedFd, Op, OpAble};
use crate::{
    buf::{IoBuf, IoVecBuf},
    BufResult,
};

pub(crate) struct Write<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,
    offset: u64,

    pub(crate) buf: T,
}

impl<T: IoBuf> Op<Write<T>> {
    pub(crate) fn write_at(fd: &SharedFd, buf: T, offset: u64) -> io::Result<Op<Write<T>>> {
        Op::submit_with(Write {
            fd: fd.clone(),
            offset,
            buf,
        })
    }

    pub(crate) async fn write(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.meta.result.map(|v| v as _), complete.data.buf)
    }
}

impl<T: IoBuf> OpAble for Write<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::Write::new(
            types::Fd(self.fd.raw_fd()),
            self.buf.read_ptr(),
            self.buf.bytes_init() as _,
        )
        .offset(self.offset)
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
        let seek_offset = libc::off_t::try_from(self.offset)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "offset too big"))?;
        #[cfg(not(target_os = "macos"))]
        return syscall_u32!(pwrite64(
            fd,
            self.buf.read_ptr() as _,
            self.buf.bytes_init(),
            seek_offset
        ));

        #[cfg(target_os = "macos")]
        return syscall_u32!(pwrite(
            fd,
            self.buf.read_ptr() as _,
            self.buf.bytes_init(),
            seek_offset
        ));
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

    #[allow(unused)]
    pub(crate) fn writev_raw(fd: &SharedFd, buf_vec: T) -> WriteVec<T> {
        WriteVec {
            fd: fd.clone(),
            buf_vec,
        }
    }

    pub(crate) async fn write(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.meta.result.map(|v| v as _), complete.data.buf_vec)
    }
}

impl<T: IoVecBuf> OpAble for WriteVec<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        let ptr = self.buf_vec.read_iovec_ptr() as *const _;
        let len = self.buf_vec.read_iovec_len() as _;
        opcode::Writev::new(types::Fd(self.fd.raw_fd()), ptr, len).build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd
            .registered_index()
            .map(|idx| (Direction::Write, idx))
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall_u32!(writev(
            self.fd.raw_fd(),
            self.buf_vec.read_iovec_ptr(),
            self.buf_vec.read_iovec_len().min(i32::MAX as usize) as _
        ))
    }
}
