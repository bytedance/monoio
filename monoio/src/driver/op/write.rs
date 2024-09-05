use std::io;
#[cfg(all(unix, any(feature = "legacy", feature = "poll-io")))]
use std::os::unix::prelude::AsRawFd;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(all(windows, any(feature = "legacy", feature = "poll-io")))]
use windows_sys::Win32::{Foundation::TRUE, Storage::FileSystem::WriteFile};

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
    /// Refers to https://docs.rs/io-uring/latest/io_uring/opcode/struct.Write.html.
    ///
    /// If `offset` is set to `-1`, the offset will use (and advance) the file position, like
    /// the write(2) system calls.
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

    pub(crate) fn write(fd: SharedFd, buf: T) -> io::Result<Op<Write<T>>> {
        Op::submit_with(Write {
            fd,
            offset: -1i64 as u64,
            buf,
        })
    }

    pub(crate) async fn result(self) -> BufResult<usize, T> {
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
        use crate::syscall_u32;

        let fd = self.fd.as_raw_fd();

        let mut seek_offset = -1;

        if -1i64 as u64 != self.offset {
            seek_offset = libc::off_t::try_from(self.offset)
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "offset too big"))?;
        }

        if seek_offset == -1 {
            syscall_u32!(write(fd, self.buf.read_ptr() as _, self.buf.bytes_init()))
        } else {
            syscall_u32!(pwrite(
                fd,
                self.buf.read_ptr() as _,
                self.buf.bytes_init(),
                seek_offset as _
            ))
        }
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        use windows_sys::Win32::{
            Foundation::{GetLastError, ERROR_HANDLE_EOF},
            System::IO::OVERLAPPED,
        };

        let fd = self.fd.raw_handle() as _;
        let seek_offset = self.offset;
        let mut bytes_write = 0;

        let ret = unsafe {
            // see https://learn.microsoft.com/zh-cn/windows/win32/api/fileapi/nf-fileapi-readfile
            if seek_offset as i64 != -1 {
                let mut overlapped: OVERLAPPED = std::mem::zeroed();
                overlapped.Anonymous.Anonymous.Offset = seek_offset as u32; // Lower 32 bits of the offset
                overlapped.Anonymous.Anonymous.OffsetHigh = (seek_offset >> 32) as u32; // Higher 32 bits of the offset

                WriteFile(
                    fd,
                    self.buf.read_ptr(),
                    self.buf.bytes_init() as u32,
                    &mut bytes_write,
                    &overlapped as *const _ as *mut _,
                )
            } else {
                WriteFile(
                    fd,
                    self.buf.read_ptr(),
                    self.buf.bytes_init() as u32,
                    &mut bytes_write,
                    std::ptr::null_mut(),
                )
            }
        };

        if ret == TRUE {
            return Ok(bytes_write);
        }

        match unsafe { GetLastError() } {
            ERROR_HANDLE_EOF => Ok(bytes_write),
            error => Err(io::Error::from_raw_os_error(error as _)),
        }
    }
}

pub(crate) struct WriteVec<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    fd: SharedFd,
    /// Refers to https://docs.rs/io-uring/latest/io_uring/opcode/struct.Write.html.
    ///
    /// If `offset` is set to `-1`, the offset will use (and advance) the file position, like
    /// the writev(2) system calls.
    offset: u64,
    pub(crate) buf_vec: T,
}

impl<T: IoVecBuf> Op<WriteVec<T>> {
    pub(crate) fn writev(fd: SharedFd, buf_vec: T) -> io::Result<Self> {
        Op::submit_with(WriteVec {
            fd,
            offset: -1i64 as u64,
            buf_vec,
        })
    }

    pub(crate) fn writev_raw(fd: &SharedFd, buf_vec: T) -> WriteVec<T> {
        WriteVec {
            fd: fd.clone(),
            offset: -1i64 as u64,
            buf_vec,
        }
    }

    pub(crate) async fn result(self) -> BufResult<usize, T> {
        let complete = self.await;
        (complete.meta.result.map(|v| v as _), complete.data.buf_vec)
    }
}

impl<T: IoVecBuf> OpAble for WriteVec<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        let ptr = self.buf_vec.read_iovec_ptr() as *const _;
        let len = self.buf_vec.read_iovec_len() as _;
        opcode::Writev::new(types::Fd(self.fd.raw_fd()), ptr, len)
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
        use crate::syscall_u32;

        let fd = self.fd.raw_fd();
        let mut seek_offset = -1;

        if -1i64 as u64 != self.offset {
            seek_offset = libc::off_t::try_from(self.offset)
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "offset too big"))?;
        }

        if seek_offset == -1 {
            syscall_u32!(writev(
                fd,
                self.buf_vec.read_iovec_ptr(),
                self.buf_vec.read_iovec_len().min(i32::MAX as usize) as _
            ))
        } else {
            syscall_u32!(pwritev(
                fd,
                self.buf_vec.read_iovec_ptr(),
                self.buf_vec.read_iovec_len().min(i32::MAX as usize) as _,
                seek_offset
            ))
        }
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        // There is no `writev` like syscall of file on windows, but this will be used to send
        // socket message.

        use windows_sys::Win32::Networking::WinSock::{WSAGetLastError, WSASend, WSAESHUTDOWN};

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
            0 => Ok(bytes_sent),
            _ => {
                let error = unsafe { WSAGetLastError() };
                if error == WSAESHUTDOWN {
                    Ok(0)
                } else {
                    Err(io::Error::from_raw_os_error(error))
                }
            }
        }
    }
}
