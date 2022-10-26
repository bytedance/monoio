use std::future::Future;

use crate::{
    buf::{IoBufMut, IoVecBufMut, RawBuf},
    BufResult,
};

/// AsyncReadRent: async read with a ownership of a buffer
pub trait AsyncReadRent {
    /// The future of read Result<size, buffer>
    type ReadFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoBufMut + 'a;
    /// The future of readv Result<size, buffer>
    type ReadvFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoVecBufMut + 'a;

    /// Same as read(2)
    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T>;
    /// Same as readv(2)
    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T>;
}

/// AsyncReadRentAt: async read with a ownership of a buffer and a position
pub trait AsyncReadRentAt {
    /// The future of Result<size, buffer>
    type Future<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Same as Read(2)
    fn read_at<T: IoBufMut>(&mut self, buf: T, pos: usize) -> Self::Future<'_, T>;
}

impl<A: ?Sized + AsyncReadRent> AsyncReadRent for &mut A {
    type ReadFuture<'a, T> = A::ReadFuture<'a, T>
    where
        Self: 'a,
        T: IoBufMut + 'a;

    type ReadvFuture<'a, T> = A::ReadvFuture<'a, T>
    where
        Self: 'a,
        T: IoVecBufMut + 'a;

    #[inline]
    fn read<T: IoBufMut>(&mut self, buf: T) -> Self::ReadFuture<'_, T> {
        (**self).read(buf)
    }

    #[inline]
    fn readv<T: IoVecBufMut>(&mut self, buf: T) -> Self::ReadvFuture<'_, T> {
        (**self).readv(buf)
    }
}

impl AsyncReadRent for &[u8] {
    type ReadFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: IoBufMut + 'a, Self: 'a;
    type ReadvFuture<'a, B> = impl std::future::Future<Output = crate::BufResult<usize, B>> where
        B: IoVecBufMut + 'a, Self: 'a;

    fn read<T: IoBufMut>(&mut self, mut buf: T) -> Self::ReadFuture<'_, T> {
        let amt = std::cmp::min(self.len(), buf.bytes_total());
        let (a, b) = self.split_at(amt);
        unsafe {
            buf.write_ptr().copy_from_nonoverlapping(a.as_ptr(), amt);
            buf.set_init(amt);
        }
        *self = b;
        async move { (Ok(amt), buf) }
    }

    fn readv<T: IoVecBufMut>(&mut self, mut buf: T) -> Self::ReadvFuture<'_, T> {
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
