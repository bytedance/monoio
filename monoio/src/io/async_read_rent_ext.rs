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
    A: AsyncReadRent + ?Sized,
    T: 'static + IoBufMut,
{
    type Future<'a> = impl Future<Output = BufResult<usize, T>> where A: 'a;

    fn read_exact(&self, mut buf: T) -> Self::Future<'_> {
        async move {
            let len = buf.bytes_total();
            let mut read = 0;
            while read < len {
                let slice = unsafe { buf.slice_mut_unchecked(read..len) };
                let (res, slice) = self.read(slice).await;
                buf = slice.into_inner();
                match res {
                    Ok(0) => {
                        return (
                            Err(std::io::Error::new(
                                std::io::ErrorKind::UnexpectedEof,
                                "failed to fill whole buffer",
                            )),
                            buf,
                        )
                    }
                    Ok(n) => read += n,
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => return (Err(e), buf),
                }
            }
            (Ok(read), buf)
        }
    }
}
