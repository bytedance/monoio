use std::future::Future;

use crate::{
    buf::{IoBufMut, IoVecBufMut, RawBuf},
    BufResult,
};

/// The `AsyncReadRent` trait defines asynchronous reading operations for objects that
/// implement it.
///
/// It provides a way to read bytes from a source into a buffer asynchronously,
/// which could be a file, socket, or any other byte-oriented stream.
///
/// Types that implement this trait are expected to manage asynchronous read operations,
/// allowing them to interact with other asynchronous tasks without blocking the executor.
pub trait AsyncReadRent {
    /// Reads bytes from this source into the provided buffer, returning the number of bytes read.
    ///
    /// # Return
    ///
    /// When this method returns `(Ok(n), buf)`, it guarantees that `0 <= n <= buf.len()`. A
    /// non-zero `n` means the buffer `buf` has been filled with `n` bytes of data from this source.
    /// If `n` is `0`, it can indicate one of two possibilities:
    ///
    /// 1. The reader has likely reached the end of the file and may not produce more bytes, though
    ///    it is not certain that no more bytes will ever be produced.
    /// 2. The provided buffer was 0 bytes in length.
    ///
    /// # Errors
    ///
    /// If an I/O or other error occurs, an error variant will be returned, ensuring that no bytes
    /// were read.
    fn read<T: IoBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>>;
    /// Similar to `read`, but reads data into a slice of buffers.
    ///
    /// Data is copied sequentially into each buffer, with the last buffer potentially being only
    /// partially filled. This method should behave equivalently to a single call to `read` with the
    /// buffers concatenated.
    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>>;
}

/// AsyncReadRentAt: async read with a ownership of a buffer and a position
pub trait AsyncReadRentAt {
    /// Same as pread(2)
    fn read_at<T: IoBufMut>(
        &self,
        buf: T,
        pos: usize,
    ) -> impl Future<Output = BufResult<usize, T>>;
}

impl<A: ?Sized + AsyncReadRent> AsyncReadRent for &mut A {
    #[inline]
    fn read<T: IoBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        (**self).read(buf)
    }

    #[inline]
    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        (**self).readv(buf)
    }
}

impl AsyncReadRent for &[u8] {
    fn read<T: IoBufMut>(&mut self, mut buf: T) -> impl Future<Output = BufResult<usize, T>> {
        let amt = std::cmp::min(self.len(), buf.bytes_total());
        let (a, b) = self.split_at(amt);
        unsafe {
            buf.write_ptr().copy_from_nonoverlapping(a.as_ptr(), amt);
            buf.set_init(amt);
        }
        *self = b;
        async move { (Ok(amt), buf) }
    }

    fn readv<T: IoVecBufMut>(&mut self, mut buf: T) -> impl Future<Output = BufResult<usize, T>> {
        // # Safety
        // We do it in pure sync way.
        let n = match unsafe { RawBuf::new_from_iovec_mut(&mut buf) } {
            Some(mut raw_buf) => {
                // copy from read to avoid await
                let amt = std::cmp::min(self.len(), raw_buf.bytes_total());
                let (a, b) = self.split_at(amt);
                unsafe {
                    raw_buf
                        .write_ptr()
                        .copy_from_nonoverlapping(a.as_ptr(), amt);
                    raw_buf.set_init(amt);
                }
                *self = b;
                amt
            }
            None => 0,
        };
        unsafe { buf.set_init(n) };
        async move { (Ok(n), buf) }
    }
}
