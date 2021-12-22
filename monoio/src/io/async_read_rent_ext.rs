use super::AsyncReadRent;
use crate::{buf::IoBufMut, BufResult};
use std::future::Future;

/// AsyncReadRentExt
pub trait AsyncReadRentExt<T: 'static> {
    /// The future of Result<size, buffer>
    type Future<'a>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Read until buf capacity is fulfilled
    fn read_exact(&self, buf: T) -> <Self as AsyncReadRentExt<T>>::Future<'_>;
}

impl<A, T> AsyncReadRentExt<T> for A
where
    A: AsyncReadRent,
    T: 'static + IoBufMut,
{
    type Future<'a>
    where
        A: 'a,
    = impl Future<Output = BufResult<usize, T>>;

    fn read_exact(&self, mut buf: T) -> Self::Future<'_> {
        async move {
            let len = buf.bytes_total();
            let mut read = 0;
            while read < len {
                let slice = unsafe { buf.slice_mut_unchecked(read..len) };
                let (r, slice_) = self.read(slice).await;
                buf = slice_.into_inner();
                match r {
                    Ok(r) => {
                        read += r;
                        if r == 0 {
                            return (Err(std::io::ErrorKind::UnexpectedEof.into()), buf);
                        }
                    }
                    Err(e) => return (Err(e), buf),
                }
            }
            (Ok(read), buf)
        }
    }
}
