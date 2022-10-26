use super::split::Split;
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut, IoVecWrapperMut},
    io::{AsyncReadRent, AsyncWriteRent},
};

/// Wrapped IO with given read prefix.
pub struct PrefixedReadIo<I, P> {
    io: I,
    prefix: P,

    prefix_finished: bool,
}

impl<I, P> PrefixedReadIo<I, P> {
    /// Create a PrefixedIo with given io and read prefix.
    pub fn new(io: I, prefix: P) -> Self {
        Self {
            io,
            prefix,
            prefix_finished: false,
        }
    }

    /// If the prefix has read to eof
    pub fn prefix_finished(&self) -> bool {
        self.prefix_finished
    }

    /// Into inner
    pub fn into_inner(self) -> I {
        self.io
    }
}

impl<I: AsyncReadRent, P: std::io::Read> AsyncReadRent for PrefixedReadIo<I, P> {
    type ReadFuture<'a, T> = impl std::future::Future<Output = crate::BufResult<usize, T>> + 'a
    where
        T: IoBufMut + 'a, Self: 'a;

    type ReadvFuture<'a, T> = impl std::future::Future<Output = crate::BufResult<usize, T>> + 'a
    where
        T: IoVecBufMut + 'a, Self: 'a;

    fn read<T: IoBufMut>(&mut self, mut buf: T) -> Self::ReadFuture<'_, T> {
        async move {
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
    }

    fn readv<T: IoVecBufMut>(&mut self, mut buf: T) -> Self::ReadvFuture<'_, T> {
        async move {
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
}

impl<I: AsyncWriteRent, P> AsyncWriteRent for PrefixedReadIo<I, P> {
    type WriteFuture<'a, T> = I::WriteFuture<'a, T> where
    T: IoBuf + 'a, Self: 'a;

    type WritevFuture<'a, T>= I::WritevFuture<'a, T> where
    T: IoVecBuf + 'a, Self: 'a;

    type FlushFuture<'a> = I::FlushFuture<'a> where Self: 'a;

    type ShutdownFuture<'a> = I::ShutdownFuture<'a> where Self: 'a;

    #[inline]
    fn write<T: IoBuf>(&mut self, buf: T) -> Self::WriteFuture<'_, T> {
        self.io.write(buf)
    }

    #[inline]
    fn writev<T: IoVecBuf>(&mut self, buf_vec: T) -> Self::WritevFuture<'_, T> {
        self.io.writev(buf_vec)
    }

    #[inline]
    fn flush(&mut self) -> Self::FlushFuture<'_> {
        self.io.flush()
    }

    #[inline]
    fn shutdown(&mut self) -> Self::ShutdownFuture<'_> {
        self.io.shutdown()
    }
}

/// implement unsafe Split for PrefixedReadIo, it's `safe`
/// because read/write are independent, we can safely split them into two I/O parts.
unsafe impl<I, P> Split for PrefixedReadIo<I, P> where I: Split {}
