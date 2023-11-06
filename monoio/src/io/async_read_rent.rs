use std::future::Future;

use crate::{
    buf::{IoBufMut, IoVecBufMut, RawBuf},
    BufResult,
};

/// AsyncReadRent: async read with a ownership of a buffer
pub trait AsyncReadRent {
    /// Same as read(2)
    fn read<T: IoBufMut>(&mut self, buf: T) -> impl Future<Output = BufResult<usize, T>>;
    /// Same as readv(2)
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
