use std::future::Future;

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
