use std::{future::Future, io::Cursor};

use crate::{
    buf::{IoBufMut, IoVecBufMut},
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
        &mut self,
        buf: T,
        pos: usize,
    ) -> impl Future<Output = BufResult<usize, T>>;
}

impl<A: ?Sized + AsyncReadRentAt> AsyncReadRentAt for &mut A {
    #[inline]
    fn read_at<T: IoBufMut>(
        &mut self,
        buf: T,
        pos: usize,
    ) -> impl Future<Output = BufResult<usize, T>> {
        (**self).read_at(buf, pos)
    }
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
        std::future::ready((Ok(amt), buf))
    }

    fn readv<T: IoVecBufMut>(&mut self, mut buf: T) -> impl Future<Output = BufResult<usize, T>> {
        let mut sum = 0;
        {
            #[cfg(windows)]
            let buf_slice = unsafe {
                std::slice::from_raw_parts_mut(buf.write_wsabuf_ptr(), buf.write_wsabuf_len())
            };
            #[cfg(unix)]
            let buf_slice = unsafe {
                std::slice::from_raw_parts_mut(buf.write_iovec_ptr(), buf.write_iovec_len())
            };
            for buf in buf_slice {
                #[cfg(windows)]
                let amt = std::cmp::min(self.len(), buf.len as usize);
                #[cfg(unix)]
                let amt = std::cmp::min(self.len(), buf.iov_len);

                let (a, b) = self.split_at(amt);
                // # Safety
                // The pointer is valid.
                unsafe {
                    #[cfg(windows)]
                    buf.buf
                        .cast::<u8>()
                        .copy_from_nonoverlapping(a.as_ptr(), amt);
                    #[cfg(unix)]
                    buf.iov_base
                        .cast::<u8>()
                        .copy_from_nonoverlapping(a.as_ptr(), amt);
                }
                *self = b;
                sum += amt;

                if self.is_empty() {
                    break;
                }
            }
        }

        unsafe { buf.set_init(sum) };
        std::future::ready((Ok(sum), buf))
    }
}

impl<T: AsRef<[u8]>> AsyncReadRent for Cursor<T> {
    async fn read<B: IoBufMut>(&mut self, buf: B) -> BufResult<usize, B> {
        let pos = self.position();
        let slice: &[u8] = (*self).get_ref().as_ref();

        if pos > slice.len() as u64 {
            return (Ok(0), buf);
        }

        (&slice[pos as usize..]).read(buf).await
    }

    async fn readv<B: IoVecBufMut>(&mut self, buf: B) -> BufResult<usize, B> {
        let pos = self.position();
        let slice: &[u8] = (*self).get_ref().as_ref();

        if pos > slice.len() as u64 {
            return (Ok(0), buf);
        }

        (&slice[pos as usize..]).readv(buf).await
    }
}

impl<T: ?Sized + AsyncReadRent> AsyncReadRent for Box<T> {
    #[inline]
    fn read<B: IoBufMut>(&mut self, buf: B) -> impl Future<Output = BufResult<usize, B>> {
        (**self).read(buf)
    }

    #[inline]
    fn readv<B: IoVecBufMut>(&mut self, buf: B) -> impl Future<Output = BufResult<usize, B>> {
        (**self).readv(buf)
    }
}
