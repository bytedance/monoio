use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(all(windows, any(feature = "legacy", feature = "poll-io")))]
use {
    crate::syscall,
    std::ffi::c_void,
    std::os::windows::io::AsRawSocket,
    windows_sys::Win32::Networking::WinSock::{recv, WSAGetLastError, WSARecv, SOCKET_ERROR},
};
#[cfg(all(unix, any(feature = "legacy", feature = "poll-io")))]
use {crate::syscall_u32, std::os::unix::prelude::AsRawFd};

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use crate::driver::ready::Direction;
use crate::{
    buf::{IoBufMut, IoVecBufMut},
    BufResult,
};

pub(crate) struct Read<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,
    offset: u64,

    /// Reference to the in-flight buffer.
    pub(crate) buf: T,
}

impl<T: IoBufMut> Op<Read<T>> {
    pub(crate) fn read_at(fd: &SharedFd, buf: T, offset: u64) -> io::Result<Op<Read<T>>> {
        Op::submit_with(Read {
            fd: fd.clone(),
            offset,
            buf,
        })
    }

    pub(crate) async fn read(self) -> BufResult<usize, T> {
        let complete = self.await;

        // Convert the operation result to `usize`
        let res = complete.meta.result.map(|v| v as usize);
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

impl<T: IoBufMut> OpAble for Read<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::Read::new(
            types::Fd(self.fd.raw_fd()),
            self.buf.write_ptr(),
            self.buf.bytes_total() as _,
        )
        .offset(self.offset)
        .build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd.registered_index().map(|idx| (Direction::Read, idx))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_fd();
        let seek_offset = libc::off_t::try_from(self.offset)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "offset too big"))?;
        #[cfg(not(target_os = "macos"))]
        return syscall_u32!(pread64(
            fd,
            self.buf.write_ptr() as _,
            self.buf.bytes_total(),
            seek_offset as _
        ));

        #[cfg(target_os = "macos")]
        return syscall_u32!(pread(
            fd,
            self.buf.write_ptr() as _,
            self.buf.bytes_total(),
            seek_offset
        ));
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_socket();
        let seek_offset = libc::off_t::try_from(self.offset)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "offset too big"))?;
        syscall!(
            recv(
                fd as _,
                (self.buf.write_ptr().cast::<c_void>() as usize + seek_offset as usize)
                    as *mut c_void as *mut _,
                self.buf.bytes_total() as i32 - seek_offset,
                0
            ),
            PartialOrd::ge,
            0
        )
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
    pub(crate) fn readv(fd: SharedFd, buf_vec: T) -> io::Result<Self> {
        Op::submit_with(ReadVec { fd, buf_vec })
    }

    pub(crate) async fn read(self) -> BufResult<usize, T> {
        let complete = self.await;
        let res = complete.meta.result.map(|v| v as _);
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

impl<T: IoVecBufMut> OpAble for ReadVec<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        let ptr = self.buf_vec.write_iovec_ptr() as _;
        let len = self.buf_vec.write_iovec_len() as _;
        opcode::Readv::new(types::Fd(self.fd.raw_fd()), ptr, len).build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd.registered_index().map(|idx| (Direction::Read, idx))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall_u32!(readv(
            self.fd.raw_fd(),
            self.buf_vec.write_iovec_ptr(),
            self.buf_vec.write_iovec_len().min(i32::MAX as usize) as _
        ))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let mut bytes_recved = 0;
        let ret = unsafe {
            WSARecv(
                self.fd.raw_socket() as _,
                self.buf_vec.write_wsabuf_ptr(),
                self.buf_vec.write_wsabuf_len() as _,
                &mut bytes_recved,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                None,
            )
        };
        match ret {
            0 => return Err(std::io::ErrorKind::WouldBlock.into()),
            SOCKET_ERROR => {
                let error = unsafe { WSAGetLastError() };
                return Err(std::io::Error::from_raw_os_error(error));
            }
            _ => (),
        }
        Ok(bytes_recved)
    }
}
