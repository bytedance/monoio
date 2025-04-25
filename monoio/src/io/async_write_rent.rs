use std::future::Future;
use std::io::Cursor;
use std::io::Write;

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

#[cfg(unix)]
fn iovecs_to_slices(iov_ptr: *const libc::iovec, iov_len: usize) -> Vec<&'static [u8]> {
    unsafe {
        std::slice::from_raw_parts(iov_ptr, iov_len)
            .iter()
            .map(|iov| std::slice::from_raw_parts(iov.iov_base as *const u8, iov.iov_len))
            .collect()
    }
}

#[cfg(windows)]
fn wsabufs_to_slices(
    wsabuf_ptr: *const windows_sys::Win32::Networking::WinSock::WSABUF,
    wsabuf_len: usize,
) -> Vec<&'static [u8]> {
    unsafe {
        std::slice::from_raw_parts(wsabuf_ptr, wsabuf_len)
            .iter()
            .map(|wsabuf| std::slice::from_raw_parts(wsabuf.buf as *const u8, wsabuf.len as usize))
            .collect()
    }
}

// Helper function for cursor writev logic
fn cursor_writev_logic<C, T>(writer: &mut C, buf_vec: T) -> BufResult<usize, T>
where
    C: std::io::Write + ?Sized, // The writer needs to implement std::io::Write
    T: IoVecBuf,
{
    #[cfg(unix)]
    {
        let iovecs = iovecs_to_slices(buf_vec.read_iovec_ptr(), buf_vec.read_iovec_len());
        let iovec_refs: Vec<std::io::IoSlice<'_>> =
            iovecs.iter().map(|s| std::io::IoSlice::new(s)).collect();
        match std::io::Write::write_vectored(writer, &iovec_refs) {
            Ok(n) => (Ok(n), buf_vec),
            Err(e) => (Err(e), buf_vec),
        }
    }
    #[cfg(windows)]
    {
        let wsabufs = wsabufs_to_slices(buf_vec.read_wsabuf_ptr(), buf_vec.read_wsabuf_len());
        let iovec_refs: Vec<std::io::IoSlice<'_>> =
            wsabufs.iter().map(|s| std::io::IoSlice::new(s)).collect();
        match std::io::Write::write_vectored(writer, &iovec_refs) {
            Ok(n) => (Ok(n), buf_vec),
            Err(e) => (Err(e), buf_vec),
        }
    }
}

impl AsyncWriteRent for Vec<u8> {
    fn write<T: IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        let slice = buf.as_slice();
        self.extend_from_slice(slice);
        let len = slice.len();
        std::future::ready((Ok(len), buf))
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        #[cfg(unix)]
        {
            let iovecs = unsafe {
                std::slice::from_raw_parts(buf_vec.read_iovec_ptr(), buf_vec.read_iovec_len())
            };
            let total_len: usize = iovecs.iter().map(|iov| iov.iov_len).sum();
            self.reserve(total_len);
            let mut written = 0;
            for slice in iovecs_to_slices(buf_vec.read_iovec_ptr(), buf_vec.read_iovec_len()) {
                self.extend_from_slice(slice);
                written += slice.len();
            }
            std::future::ready((Ok(written), buf_vec))
        }
        #[cfg(windows)]
        {
            let wsabufs = unsafe {
                std::slice::from_raw_parts(buf_vec.read_wsabuf_ptr(), buf_vec.read_wsabuf_len())
            };
            let total_len: usize = wsabufs.iter().map(|wsabuf| wsabuf.len as usize).sum();
            self.reserve(total_len);
            let mut written = 0;
            for slice in wsabufs_to_slices(buf_vec.read_wsabuf_ptr(), buf_vec.read_wsabuf_len()) {
                self.extend_from_slice(slice);
                written += slice.len();
            }
            std::future::ready((Ok(written), buf_vec))
        }
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

impl AsyncWriteRent for Cursor<&mut Vec<u8>> {
    async fn write<T: IoBuf>(&mut self, buf: T) -> BufResult<usize, T> {
        let slice = buf.as_slice();
        match Write::write(self, slice) {
            Ok(n) => (Ok(n), buf),
            Err(e) => (Err(e), buf),
        }
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        std::future::ready(cursor_writev_logic(self, buf_vec))
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

impl AsyncWriteRent for Cursor<&mut [u8]> {
    async fn write<T: IoBuf>(&mut self, buf: T) -> BufResult<usize, T> {
        let slice = buf.as_slice();
        match Write::write(self, slice) {
            Ok(n) => (Ok(n), buf),
            Err(e) => (Err(e), buf),
        }
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        std::future::ready(cursor_writev_logic(self, buf_vec))
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

impl AsyncWriteRent for Cursor<Box<[u8]>> {
    async fn write<T: IoBuf>(&mut self, buf: T) -> BufResult<usize, T> {
        let slice = buf.as_slice();
        match Write::write(self, slice) {
            Ok(n) => (Ok(n), buf),
            Err(e) => (Err(e), buf),
        }
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        std::future::ready(cursor_writev_logic(self, buf_vec))
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

impl AsyncWriteRent for Cursor<Vec<u8>> {
    async fn write<T: IoBuf>(&mut self, buf: T) -> BufResult<usize, T> {
        let slice = buf.as_slice();
        match Write::write(self, slice) {
            Ok(n) => (Ok(n), buf),
            Err(e) => (Err(e), buf),
        }
    }

    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        std::future::ready(cursor_writev_logic(self, buf_vec))
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
