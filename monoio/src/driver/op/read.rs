use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(all(unix, any(feature = "legacy", feature = "poll-io")))]
use {crate::syscall_u32, std::os::unix::prelude::AsRawFd};
#[cfg(all(windows, any(feature = "legacy", feature = "poll-io")))]
use {
    std::ffi::c_void,
    windows_sys::Win32::{Foundation::TRUE, Storage::FileSystem::ReadFile},
};

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

    pub(crate) fn read(fd: SharedFd, buf: T) -> io::Result<Op<Read<T>>> {
        Op::submit_with(Read {
            fd,
            buf,
            // Refers to https://docs.rs/io-uring/latest/io_uring/opcode/struct.Write.html.
            // If `offset` is set to `-1`, the offset will use (and advance) the file position, like
            // the read(2) and write(2) system calls.
            offset: -1i64 as u64,
        })
    }

    pub(crate) async fn result(self) -> BufResult<usize, T> {
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

        let mut seek_offset = -1;

        if -1i64 as u64 != self.offset {
            seek_offset = libc::off_t::try_from(self.offset)
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "offset too big"))?;
        }

        if seek_offset == -1 {
            syscall_u32!(read(fd, self.buf.write_ptr() as _, self.buf.bytes_total()))
        } else {
            syscall_u32!(pread(
                fd,
                self.buf.write_ptr() as _,
                self.buf.bytes_total(),
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

        let mut bytes_read = 0;
        let ret = unsafe {
            // see https://learn.microsoft.com/zh-cn/windows/win32/api/fileapi/nf-fileapi-readfile
            if seek_offset as i64 != -1 {
                let mut overlapped: OVERLAPPED = std::mem::zeroed();
                overlapped.Anonymous.Anonymous.Offset = seek_offset as u32; // Lower 32 bits of the offset
                overlapped.Anonymous.Anonymous.OffsetHigh = (seek_offset >> 32) as u32; // Higher 32 bits of the offset

                ReadFile(
                    fd,
                    self.buf.write_ptr().cast::<c_void>(),
                    self.buf.bytes_total() as u32,
                    &mut bytes_read,
                    &overlapped as *const _ as *mut _,
                )
            } else {
                ReadFile(
                    fd,
                    self.buf.write_ptr().cast::<c_void>(),
                    self.buf.bytes_total() as u32,
                    &mut bytes_read,
                    std::ptr::null_mut(),
                )
            }
        };

        if ret == TRUE {
            return Ok(bytes_read);
        }

        match unsafe { GetLastError() } {
            ERROR_HANDLE_EOF => Ok(bytes_read),
            error => Err(io::Error::from_raw_os_error(error as _)),
        }
    }
}

pub(crate) struct ReadVec<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,
    offset: u64,

    /// Reference to the in-flight buffer.
    pub(crate) buf_vec: T,
}

impl<T: IoVecBufMut> Op<ReadVec<T>> {
    pub(crate) fn readv(fd: SharedFd, buf_vec: T) -> io::Result<Self> {
        Op::submit_with(ReadVec {
            fd,
            // Refers to https://docs.rs/io-uring/latest/io_uring/opcode/struct.Write.html.
            // If `offset` is set to `-1`, the offset will use (and advance) the file position, like
            // the readv(2) system calls.
            offset: -1i64 as u64,
            buf_vec,
        })
    }

    pub(crate) async fn result(self) -> BufResult<usize, T> {
        let complete = self.await;
        let res = complete.meta.result.map(|v| v as _);
        let mut buf_vec = complete.data.buf_vec;

        if let Ok(n) = res {
            // Safety: the kernel wrote `n` bytes to the buffer.
            unsafe { buf_vec.set_init(n) };
        }
        (res, buf_vec)
    }
}

impl<T: IoVecBufMut> OpAble for ReadVec<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        let ptr = self.buf_vec.write_iovec_ptr() as _;
        let len = self.buf_vec.write_iovec_len() as _;
        opcode::Readv::new(types::Fd(self.fd.raw_fd()), ptr, len)
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
        syscall_u32!(readv(
            self.fd.raw_fd(),
            self.buf_vec.write_iovec_ptr(),
            self.buf_vec.write_iovec_len().min(i32::MAX as usize) as _
        ))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        // There is no `readv` like syscall of file on windows, but this will be used to send
        // socket message.

        use windows_sys::Win32::Networking::WinSock::{WSAGetLastError, WSARecv, WSAESHUTDOWN};

        let mut nread = 0;
        let mut flags = 0;
        let ret = unsafe {
            WSARecv(
                self.fd.raw_socket() as _,
                self.buf_vec.write_wsabuf_ptr(),
                self.buf_vec.write_wsabuf_len().min(u32::MAX as usize) as _,
                &mut nread,
                &mut flags,
                std::ptr::null_mut(),
                None,
            )
        };
        match ret {
            0 => Ok(nread),
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
