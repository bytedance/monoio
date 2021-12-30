use super::AsyncWriteRent;
use crate::{buf::IoBuf, BufResult};
use std::future::Future;

/// AsyncWriteRentExt
pub trait AsyncWriteRentExt<T: 'static> {
    /// The future of Result<size, buffer>
    type Future<'a>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Write all
    fn write_all(&self, buf: T) -> <Self as AsyncWriteRentExt<T>>::Future<'_>;
}

impl<A, T> AsyncWriteRentExt<T> for A
where
    A: AsyncWriteRent + ?Sized,
    T: 'static + IoBuf,
{
    type Future<'a>
    where
        A: 'a,
    = impl Future<Output = BufResult<usize, T>>;

    fn write_all(&self, mut buf: T) -> Self::Future<'_> {
        async move {
            let len = buf.bytes_init();
            let mut written = 0;
            while written < len {
                let slice = unsafe { buf.slice_unchecked(written..len) };
                let (res, slice) = self.write(slice).await;
                buf = slice.into_inner();
                match res {
                    Ok(0) => {
                        return (
                            Err(std::io::Error::new(
                                std::io::ErrorKind::WriteZero,
                                "failed to write whole buffer",
                            )),
                            buf,
                        )
                    }
                    Ok(n) => written += n,
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => return (Err(e), buf),
                }
            }
            (Ok(written), buf)
        }
    }
}
