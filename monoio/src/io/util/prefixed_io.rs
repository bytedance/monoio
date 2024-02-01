use std::future::Future;

use super::{split::Split, CancelHandle};
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut, IoVecWrapperMut},
    io::{AsyncReadRent, AsyncWriteRent, CancelableAsyncReadRent, CancelableAsyncWriteRent},
    BufResult,
};

/// PrefixedReadIO facilitates the addition of a prefix to an IO stream,
/// enabling stream rewinding and peeking capabilities.
/// Subsequent reads will preserve access to the original stream contents.
/// ```
/// # use monoio::io::PrefixedReadIo;
/// # use monoio::io::{AsyncReadRent, AsyncWriteRent, AsyncReadRentExt};
///
/// async fn demo<T>(mut stream: T)
/// where
///     T: AsyncReadRent + AsyncWriteRent,
/// {
///     // let stream = b"hello world";
///     let buf = vec![0 as u8; 6];
///     let (_, buf) = stream.read_exact(buf).await;
///     assert_eq!(buf, b"hello ");
///
///     let prefix_buf = std::io::Cursor::new(buf);
///     let mut pio = PrefixedReadIo::new(stream, prefix_buf);
///
///     let buf = vec![0 as u8; 11];
///     let (_, buf) = pio.read_exact(buf).await;
///     assert_eq!(buf, b"hello world");
/// }
/// ```
pub struct PrefixedReadIo<I, P> {
    io: I,
    prefix: P,

    prefix_finished: bool,
}

impl<I, P> PrefixedReadIo<I, P> {
    /// Create a PrefixedIo with given io and read prefix.
    pub const fn new(io: I, prefix: P) -> Self {
        Self {
            io,
            prefix,
            prefix_finished: false,
        }
    }

    /// If the prefix has read to eof
    pub const fn prefix_finished(&self) -> bool {
        self.prefix_finished
    }

    /// Into inner
    #[inline]
    pub fn into_inner(self) -> I {
        self.io
    }
}

impl<I: AsyncReadRent, P: std::io::Read> AsyncReadRent for PrefixedReadIo<I, P> {
    async fn read<T: IoBufMut>(&mut self, mut buf: T) -> BufResult<usize, T> {
        if buf.bytes_total() == 0 {
            return (Ok(0), buf);
        }
        if !self.prefix_finished {
            let slice = unsafe {
                &mut *std::ptr::slice_from_raw_parts_mut(buf.write_ptr(), buf.bytes_total())
            };
            match self.prefix.read(slice) {
                Ok(0) => {
                    // prefix finished
                    self.prefix_finished = true;
                }
                Ok(n) => {
                    unsafe { buf.set_init(n) };
                    return (Ok(n), buf);
                }
                Err(e) => {
                    return (Err(e), buf);
                }
            }
        }
        // prefix eof now, read io directly
        self.io.read(buf).await
    }

    async fn readv<T: IoVecBufMut>(&mut self, mut buf: T) -> BufResult<usize, T> {
        let slice = match IoVecWrapperMut::new(buf) {
            Ok(slice) => slice,
            Err(buf) => return (Ok(0), buf),
        };

        let (result, slice) = self.read(slice).await;
        buf = slice.into_inner();
        if let Ok(n) = result {
            unsafe { buf.set_init(n) };
        }
        (result, buf)
    }
}

impl<I: CancelableAsyncReadRent, P: std::io::Read> CancelableAsyncReadRent
    for PrefixedReadIo<I, P>
{
    async fn cancelable_read<T: IoBufMut>(
        &mut self,
        mut buf: T,
        c: CancelHandle,
    ) -> crate::BufResult<usize, T> {
        if buf.bytes_total() == 0 {
            return (Ok(0), buf);
        }
        if !self.prefix_finished {
            let slice = unsafe {
                &mut *std::ptr::slice_from_raw_parts_mut(buf.write_ptr(), buf.bytes_total())
            };
            match self.prefix.read(slice) {
                Ok(0) => {
                    // prefix finished
                    self.prefix_finished = true;
                }
                Ok(n) => {
                    unsafe { buf.set_init(n) };
                    return (Ok(n), buf);
                }
                Err(e) => {
                    return (Err(e), buf);
                }
            }
        }
        // prefix eof now, read io directly
        self.io.cancelable_read(buf, c).await
    }

    async fn cancelable_readv<T: IoVecBufMut>(
        &mut self,
        mut buf: T,
        c: CancelHandle,
    ) -> crate::BufResult<usize, T> {
        let slice = match IoVecWrapperMut::new(buf) {
            Ok(slice) => slice,
            Err(buf) => return (Ok(0), buf),
        };

        let (result, slice) = self.cancelable_read(slice, c).await;
        buf = slice.into_inner();
        if let Ok(n) = result {
            unsafe { buf.set_init(n) };
        }
        (result, buf)
    }
}

impl<I: AsyncWriteRent, P> AsyncWriteRent for PrefixedReadIo<I, P> {
    #[inline]
    fn write<T: IoBuf>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>> {
        self.io.write(buf)
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> impl Future<Output = BufResult<usize, T>> {
        self.io.writev(buf_vec)
    }

    #[inline]
    fn flush(&mut self) -> impl Future<Output = std::io::Result<()>> {
        self.io.flush()
    }

    #[inline]
    fn shutdown(&mut self) -> impl Future<Output = std::io::Result<()>> {
        self.io.shutdown()
    }
}

impl<I: CancelableAsyncWriteRent, P> CancelableAsyncWriteRent for PrefixedReadIo<I, P> {
    #[inline]
    fn cancelable_write<T: IoBuf>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>> {
        self.io.cancelable_write(buf, c)
    }

    #[inline]
    fn cancelable_writev<T: IoVecBuf>(
        &mut self,
        buf_vec: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>> {
        self.io.cancelable_writev(buf_vec, c)
    }

    #[inline]
    fn cancelable_flush(&mut self, c: CancelHandle) -> impl Future<Output = std::io::Result<()>> {
        self.io.cancelable_flush(c)
    }

    #[inline]
    fn cancelable_shutdown(
        &mut self,
        c: CancelHandle,
    ) -> impl Future<Output = std::io::Result<()>> {
        self.io.cancelable_shutdown(c)
    }
}

/// implement unsafe Split for PrefixedReadIo, it's `safe`
/// because read/write are independent, we can safely split them into two I/O parts.
unsafe impl<I, P> Split for PrefixedReadIo<I, P> where I: Split {}
