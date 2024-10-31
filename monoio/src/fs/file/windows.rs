use std::{
    mem::ManuallyDrop,
    os::windows::io::{AsRawHandle, RawHandle},
};

#[cfg(all(not(feature = "iouring"), feature = "sync"))]
pub(crate) use asyncified::*;
#[cfg(any(feature = "iouring", not(feature = "sync")))]
pub(crate) use blocking::*;
use windows_sys::Win32::Networking::WinSock::WSABUF;

use super::File;
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut},
    driver::shared_fd::SharedFd,
};

impl AsRawHandle for File {
    fn as_raw_handle(&self) -> RawHandle {
        self.fd.raw_handle()
    }
}

#[cfg(any(feature = "iouring", not(feature = "sync")))]
mod blocking {
    use super::*;
    use crate::uring_op;

    uring_op!(read<IoBufMut>(read, buf));
    uring_op!(read_at<IoBufMut>(read_at, buf, pos: u64));

    uring_op!(write<IoBuf>(write, buf));
    uring_op!(write_at<IoBuf>(write_at, buf, pos: u64));

    /// The `readv` implement on windows.
    ///
    /// Due to windows does not have syscall like `readv`, so we need to simulate it by ourself.
    ///
    /// This function is just to fill each buffer by calling the `read` function.
    pub(crate) async fn read_vectored<T: IoVecBufMut>(
        fd: SharedFd,
        mut buf_vec: T,
    ) -> crate::BufResult<usize, T> {
        // Convert the mutable buffer vector into raw pointers that can be used in unsafe operations
        let raw_bufs = buf_vec.write_wsabuf_ptr();
        let len = buf_vec.write_wsabuf_len();

        let wsabufs = unsafe { std::slice::from_raw_parts(raw_bufs, len) };

        let mut total_bytes_read = 0;

        // Iterate through each WSABUF structure and read data into it
        for wsabuf in wsabufs.iter() {
            // Safely create a Vec from the WSABUF pointer, then pass it to the read function
            let (res, _) = read(
                fd.clone(),
                ManuallyDrop::new(unsafe {
                    Vec::from_raw_parts(wsabuf.buf, wsabuf.len as usize, wsabuf.len as usize)
                }),
            )
            .await;

            // Handle the result of the read operation
            match res {
                Ok(bytes_read) => {
                    total_bytes_read += bytes_read;
                    // If fewer bytes were read than requested, stop further reads
                    if bytes_read < wsabuf.len as usize {
                        break;
                    }
                }
                Err(e) => {
                    // If an error occurs, return it along with the original buffer vector
                    return (Err(e), buf_vec);
                }
            }
        }

        // Due to `read` will init each buffer, so we do need to set buffer len here.
        // Return the total bytes read and the buffer vector
        (Ok(total_bytes_read), buf_vec)
    }

    /// The `writev` implement on windows
    ///
    /// Due to windows does not have syscall like `writev`, so we need to simulate it by ourself.
    ///
    /// This function is just to write each buffer into file by calling the `write` function.
    pub(crate) async fn write_vectored<T: IoVecBuf>(
        fd: SharedFd,
        buf_vec: T,
    ) -> crate::BufResult<usize, T> {
        // Convert the buffer vector into raw pointers that can be used in unsafe operations
        let raw_bufs = buf_vec.read_wsabuf_ptr() as *mut WSABUF;
        let len = buf_vec.read_wsabuf_len();

        let wsabufs = unsafe { std::slice::from_raw_parts(raw_bufs, len) };
        let mut total_bytes_write = 0;

        // Iterate through each WSABUF structure and write data from it
        for wsabuf in wsabufs.iter() {
            // Safely create a Vec from the WSABUF pointer, then pass it to the write function
            let (res, _) = write(
                fd.clone(),
                ManuallyDrop::new(unsafe {
                    Vec::from_raw_parts(wsabuf.buf, wsabuf.len as usize, wsabuf.len as usize)
                }),
            )
            .await;

            // Handle the result of the write operation
            match res {
                Ok(bytes_write) => {
                    total_bytes_write += bytes_write;
                    // If fewer bytes were written than requested, stop further writes
                    if bytes_write < wsabuf.len as usize {
                        break;
                    }
                }
                Err(e) => {
                    // If an error occurs, return it along with the original buffer vector
                    return (Err(e), buf_vec);
                }
            }
        }

        // Return the total bytes written and the buffer vector
        (Ok(total_bytes_write), buf_vec)
    }
}

#[cfg(all(not(feature = "iouring"), feature = "sync"))]
mod asyncified {
    use super::*;
    use crate::{
        asyncify_op,
        driver::op::{read, write},
        fs::asyncify,
    };

    asyncify_op!(R, read<IoBufMut>(read::read, IoBufMut::write_ptr, IoBufMut::bytes_total));
    asyncify_op!(R, read_at<IoBufMut>(read::read_at, IoBufMut::write_ptr, IoBufMut::bytes_total, pos: u64));

    asyncify_op!(W, write<IoBuf>(write::write, IoBuf::read_ptr, IoBuf::bytes_init));
    asyncify_op!(W, write_at<IoBuf>(write::write_at, IoBuf::read_ptr, IoBuf::bytes_init, pos: u64));

    /// The `readv` implement on windows.
    ///
    /// Due to windows does not have syscall like `readv`, so we need to simulate it by ourself.
    ///
    /// This function is just to fill each buffer by calling the `read` function.
    pub(crate) async fn read_vectored<T: IoVecBufMut>(
        fd: SharedFd,
        mut buf_vec: T,
    ) -> crate::BufResult<usize, T> {
        // Convert the mutable buffer vector into raw pointers that can be used in unsafe
        // operations
        let raw_bufs = buf_vec.write_wsabuf_ptr() as usize;
        let len = buf_vec.write_wsabuf_len();
        let fd = fd.as_raw_handle() as _;

        let res = asyncify(move || {
            let wsabufs = unsafe { std::slice::from_raw_parts(raw_bufs as *mut WSABUF, len) };

            let mut total_bytes_read = 0;

            // Iterate through each WSABUF structure and read data into it
            for wsabuf in wsabufs.iter() {
                let res = read::read(fd, wsabuf.buf, wsabuf.len as usize);

                // Handle the result of the read operation
                match res {
                    Ok(bytes_read) => {
                        let bytes_read = bytes_read.into_inner();
                        total_bytes_read += bytes_read;
                        // If fewer bytes were read than requested, stop further reads
                        if bytes_read < wsabuf.len {
                            break;
                        }
                    }
                    Err(e) => {
                        // If an error occurs, return it along with the original buffer vector
                        return Err(e);
                    }
                }
            }

            // Due to `read` will init each buffer, so we do need to set buffer len here.
            // Return the total bytes read and the buffer vector
            Ok(total_bytes_read)
        })
        .await
        .map(|n| n as usize);

        unsafe { buf_vec.set_init(*res.as_ref().unwrap_or(&0)) };

        (res, buf_vec)
    }

    /// The `writev` implement on windows
    ///
    /// Due to windows does not have syscall like `writev`, so we need to simulate it by ourself.
    ///
    /// This function is just to write each buffer into file by calling the `write` function.
    pub(crate) async fn write_vectored<T: IoVecBuf>(
        fd: SharedFd,
        buf_vec: T,
    ) -> crate::BufResult<usize, T> {
        // Convert the mutable buffer vector into raw pointers that can be used in unsafe
        // operation
        let raw_bufs = buf_vec.read_wsabuf_ptr() as usize;
        let len = buf_vec.read_wsabuf_len();
        let fd = fd.as_raw_handle() as _;

        let res = asyncify(move || {
            let wsabufs = unsafe { std::slice::from_raw_parts(raw_bufs as *mut WSABUF, len) };

            let mut total_bytes_write = 0;

            for wsabuf in wsabufs.iter() {
                let res = write::write(fd, wsabuf.buf, wsabuf.len as _);

                match res {
                    Ok(bytes_write) => {
                        let bytes_write = bytes_write.into_inner();
                        total_bytes_write += bytes_write;
                        if bytes_write < wsabuf.len {
                            break;
                        }
                    }
                    Err(e) => return Err(e),
                }
            }

            Ok(total_bytes_write)
        })
        .await
        .map(|n| n as usize);

        (res, buf_vec)
    }
}
