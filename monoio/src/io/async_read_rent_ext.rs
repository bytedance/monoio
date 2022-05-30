use super::AsyncReadRent;
use crate::{
    buf::{IoBufMut, IoVecBufMut},
    BufResult,
};
use std::future::Future;

/// AsyncReadRentExt
pub trait AsyncReadRentExt {
    /// The future of Result<size, buffer>
    type ReadExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Read until buf capacity is fulfilled
    fn read_exact<T: 'static>(&mut self, buf: T) -> Self::ReadExactFuture<'_, T>
    where
        T: 'static + IoBufMut;

    /// The future of Result<size, buffer>
    type ReadVectoredExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Readv until buf capacity is fulfilled
    fn read_vectored_exact<T: 'static>(&mut self, buf: T) -> Self::ReadVectoredExactFuture<'_, T>
    where
        T: 'static + IoVecBufMut;
}

impl<A> AsyncReadRentExt for A
where
    A: AsyncReadRent + ?Sized,
{
    type ReadExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> where A: 'a, T: 'a;

    fn read_exact<T>(&mut self, mut buf: T) -> Self::ReadExactFuture<'_, T>
    where
        T: 'static + IoBufMut,
    {
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

    type ReadVectoredExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> where A: 'a, T: 'a;

    fn read_vectored_exact<T: 'static>(
        &mut self,
        mut buf: T,
    ) -> Self::ReadVectoredExactFuture<'_, T>
    where
        T: 'static + IoVecBufMut,
    {
        let mut meta = crate::buf::write_vec_meta(&mut buf);
        let len = meta.len();
        let mut read = 0;
        async move {
            while read < len {
                let (res, meta_) = self.readv(meta).await;
                meta = meta_;
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
