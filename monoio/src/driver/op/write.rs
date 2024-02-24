use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(all(unix, any(feature = "legacy", feature = "poll-io")))]
use {crate::syscall_u32, std::os::unix::prelude::AsRawFd};
#[cfg(all(windows, any(feature = "legacy", feature = "poll-io")))]
use {
    std::os::windows::io::AsRawSocket,
    windows_sys::Win32::{
        Foundation::TRUE,
        Networking::WinSock::{WSAGetLastError, WSASend, SOCKET_ERROR},
        Storage::FileSystem::{SetFilePointer, WriteFile, FILE_CURRENT, INVALID_SET_FILE_POINTER},
    },
};

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use crate::driver::ready::Direction;
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

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd
            .registered_index()
            .map(|idx| (Direction::Write, idx))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_fd();
        let seek_offset = libc::off_t::try_from(self.offset)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "offset too big"))?;
        #[cfg(not(target_os = "macos"))]
        return syscall_u32!(pwrite64(
            fd,
            self.buf.read_ptr() as _,
            self.buf.bytes_init(),
            seek_offset as _
        ));

        #[cfg(target_os = "macos")]
        return syscall_u32!(pwrite(
            fd,
            self.buf.read_ptr() as _,
            self.buf.bytes_init(),
            seek_offset
        ));
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_socket() as _;
        let seek_offset = libc::off_t::try_from(self.offset)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "offset too big"))?;
        let mut bytes_write = 0;
        let ret = unsafe {
            // see https://learn.microsoft.com/zh-cn/windows/win32/api/fileapi/nf-fileapi-setfilepointer
            if seek_offset != 0 {
                let r = SetFilePointer(fd, seek_offset, std::ptr::null_mut(), FILE_CURRENT);
                if INVALID_SET_FILE_POINTER == r {
                    return Err(io::Error::last_os_error());
                }
            }
            // see https://learn.microsoft.com/zh-cn/windows/win32/api/fileapi/nf-fileapi-writefile
            WriteFile(
                fd,
                self.buf.read_ptr(),
                self.buf.bytes_init() as u32,
                &mut bytes_write,
                std::ptr::null_mut(),
            )
        };
        if TRUE == ret {
            Ok(bytes_write)
        } else {
            Err(io::Error::last_os_error())
        }
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

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd
            .registered_index()
            .map(|idx| (Direction::Write, idx))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall_u32!(writev(
            self.fd.raw_fd(),
            self.buf_vec.read_iovec_ptr(),
            self.buf_vec.read_iovec_len().min(i32::MAX as usize) as _
        ))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let mut bytes_sent = 0;
        let ret = unsafe {
            WSASend(
                self.fd.raw_socket() as _,
                self.buf_vec.read_wsabuf_ptr(),
                self.buf_vec.read_wsabuf_len() as _,
                &mut bytes_sent,
                0,
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
        Ok(bytes_sent)
    }
}
