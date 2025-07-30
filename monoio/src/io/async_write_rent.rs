use std::{
    future::Future,
    io::{Cursor, Write},
};

use crate::{
    buf::{IoBuf, IoVecBuf},
    BufResult,
};

/// The `AsyncWriteRent` trait provides asynchronous writing capabilities for structs
/// that implement it.
///
/// It abstracts over the concept of writing bytes asynchronously
/// to an underlying I/O object, which could be a file, socket, or any other
/// byte-oriented stream. The trait also encompasses the ability to flush buffered
/// data and to shut down the output stream cleanly.
///
/// Types implementing this trait are required to manage asynchronous I/O operations,
/// allowing for non-blocking writes. This is particularly useful in scenarios where
/// the object might need to interact with other asynchronous tasks without blocking
/// the executor.
pub trait AsyncWriteRent {
    /// Writes the contents of a buffer into this writer, returning the number of bytes written.
    ///
    /// This function attempts to write the entire buffer `buf`, but the write may not fully
    /// succeed, and it might also result in an error. A call to `write` represents *at most one*
    /// attempt to write to the underlying object.
    ///
    /// # Return
    ///
    /// When this method returns `(Ok(n), buf)`, it guarantees that `n <= buf.len()`. A return value
    /// of `0` typically indicates that the underlying object can no longer accept bytes and likely
    /// won't be able to in the future, or that the provided buffer is empty.
    ///
    /// # Errors
    ///
    /// Each `write` call may result in an I/O error, indicating the operation couldn't be
    /// completed. If an error occurs, no bytes from the buffer were written to the writer.
    ///
    /// It is **not** an error if the entire buffer could not be written to this writer.
    fn write<T: IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>>;

    /// This function attempts to write the entire contents of `buf_vec`, but the write may not
    /// fully succeed, and it might also result in an error. The bytes will be written starting at
    /// the specified offset.
    ///
    /// # Return
    ///
    /// The method returns the result of the operation along with the same array of buffers passed
    /// as an argument. A return value of `0` typically indicates that the underlying file can no
    /// longer accept bytes and likely won't be able to in the future, or that the provided buffer
    /// is empty.
    ///
    /// # Errors
    ///
    /// Each `write` call may result in an I/O error, indicating the operation couldn't be
    /// completed. If an error occurs, no bytes from the buffer were written to the writer.
    ///
    /// It is **not** considered an error if the entire buffer could not be written to this writer.
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>>;

    /// Flushes this output stream, ensuring that all buffered content is successfully written to
    /// its destination.
    ///
    /// # Errors
    ///
    /// An error occurs if not all bytes can be written due to I/O issues or if the end of the file
    /// (EOF) is reached.
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>>;

    /// Shuts down the output stream, ensuring that the value can be cleanly dropped.
    ///
    /// Similar to [`flush`], all buffered data is written to the underlying stream. After this
    /// operation completes, the caller should no longer attempt to write to the stream.
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>>;
}

/// AsyncWriteRentAt: async write with a ownership of a buffer and a position
pub trait AsyncWriteRentAt {
    /// Write buf at given offset
    fn write_at<T: IoBuf>(
        &mut self,
        buf: T,
        pos: usize,
    ) -> impl Future<Output = BufResult<usize, T>>;
}

impl<A: ?Sized + AsyncWriteRentAt> AsyncWriteRentAt for &mut A {
    #[inline]
    fn write_at<T: IoBuf>(
        &mut self,
        buf: T,
        pos: usize,
    ) -> impl Future<Output = BufResult<usize, T>> {
        (**self).write_at(buf, pos)
    }
}

impl<A: ?Sized + AsyncWriteRent> AsyncWriteRent for &mut A {
    #[inline]
    fn write<T: IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        (**self).write(buf)
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        (**self).writev(buf_vec)
    }

    #[inline]
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>> {
        (**self).flush()
    }

    #[inline]
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>> {
        (**self).shutdown()
    }
}

// Helper function for cursor writev logic
#[inline]
fn write_vectored_logic<C: std::io::Write + ?Sized, T: IoVecBuf>(
    writer: &mut C,
    buf_vec: T,
) -> BufResult<usize, T> {
    let bufs: &[std::io::IoSlice<'_>];
    #[cfg(unix)]
    {
        // SAFETY: IoSlice<'_> is repr(transparent) over libc::iovec
        bufs = unsafe {
            std::slice::from_raw_parts(
                buf_vec.read_iovec_ptr() as *const std::io::IoSlice<'_>,
                buf_vec.read_iovec_len(),
            )
        };
    }
    #[cfg(windows)]
    {
        // SAFETY: IoSlice<'_> is repr(transparent) over WSABUF
        bufs = unsafe {
            std::slice::from_raw_parts(
                buf_vec.read_wsabuf_ptr() as *const std::io::IoSlice<'_>,
                buf_vec.read_wsabuf_len(),
            )
        };
    }
    let res = std::io::Write::write_vectored(writer, bufs);
    match res {
        Ok(n) => (Ok(n), buf_vec),
        Err(e) => (Err(e), buf_vec),
    }
}

// Helper function to extend a Vec<u8> from platform-specific buffer slices
#[inline]
fn extend_vec_from_platform_bufs<P>(
    vec: &mut Vec<u8>,
    platform_bufs: &[P],
    get_ptr: fn(&P) -> *const u8,
    get_len: fn(&P) -> usize,
) -> usize {
    let mut total_bytes_to_write = 0;
    for buf_part in platform_bufs.iter() {
        total_bytes_to_write += get_len(buf_part);
    }

    if total_bytes_to_write == 0 {
        return 0;
    }
    vec.reserve(total_bytes_to_write);

    for buf_part in platform_bufs.iter() {
        let buffer_ptr = get_ptr(buf_part);
        let buffer_len = get_len(buf_part);
        if buffer_len > 0 {
            let buffer_data_slice = unsafe { std::slice::from_raw_parts(buffer_ptr, buffer_len) };
            vec.extend_from_slice(buffer_data_slice);
        }
    }
    total_bytes_to_write
}

impl AsyncWriteRent for Vec<u8> {
    fn write<T: IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        let slice = buf.as_slice();
        self.extend_from_slice(slice);
        let len = slice.len();
        std::future::ready((Ok(len), buf))
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        let total_bytes_to_write: usize;
        #[cfg(unix)]
        {
            // SAFETY: IoVecBuf guarantees valid iovec array
            let iovec_array_ptr = buf_vec.read_iovec_ptr();
            let iovec_count = buf_vec.read_iovec_len();
            let iovec_slice = unsafe { std::slice::from_raw_parts(iovec_array_ptr, iovec_count) };
            total_bytes_to_write = extend_vec_from_platform_bufs(
                self,
                iovec_slice,
                |iovec: &libc::iovec| iovec.iov_base as *const u8,
                |iovec: &libc::iovec| iovec.iov_len,
            );
        }
        #[cfg(windows)]
        {
            // SAFETY: IoVecBuf guarantees valid WSABUF array
            let wsabuf_array_ptr = buf_vec.read_wsabuf_ptr();
            let wsabuf_count = buf_vec.read_wsabuf_len();
            let wsabuf_slice =
                unsafe { std::slice::from_raw_parts(wsabuf_array_ptr, wsabuf_count) };
            total_bytes_to_write = extend_vec_from_platform_bufs(
                self,
                wsabuf_slice,
                |wsabuf: &windows_sys::Win32::Networking::WinSock::WSABUF| wsabuf.buf as *const u8,
                |wsabuf: &windows_sys::Win32::Networking::WinSock::WSABUF| wsabuf.len as usize,
            );
        }
        std::future::ready((Ok(total_bytes_to_write), buf_vec))
    }

    #[inline]
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>> {
        std::future::ready(Ok(()))
    }

    #[inline]
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>> {
        std::future::ready(Ok(()))
    }
}

impl<W> AsyncWriteRent for Cursor<W>
where
    Cursor<W>: Write + Unpin,
{
    #[inline]
    fn write<T: IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        let slice = buf.as_slice();
        std::future::ready((Write::write(self, slice), buf))
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        std::future::ready(write_vectored_logic(self, buf_vec))
    }

    #[inline]
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>> {
        std::future::ready(Write::flush(self))
    }

    #[inline]
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>> {
        // Cursor is in-memory, flush is a no-op, so shutdown is also a no-op.
        std::future::ready(Ok(()))
    }
}

impl<T: ?Sized + AsyncWriteRent + Unpin> AsyncWriteRent for Box<T> {
    #[inline]
    fn write<B: IoBuf>(&mut self, buf: B) -> impl Future<Output = BufResult<usize, B>> {
        (**self).write(buf)
    }

    #[inline]
    fn writev<B: IoVecBuf>(&mut self, buf_vec: B) -> impl Future<Output = BufResult<usize, B>> {
        (**self).writev(buf_vec)
    }

    #[inline]
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>> {
        (**self).flush()
    }

    #[inline]
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>> {
        (**self).shutdown()
    }
}
