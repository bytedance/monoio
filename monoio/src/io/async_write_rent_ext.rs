use std::future::Future;

use crate::{
    buf::{IoBuf, IoVecBuf, Slice},
    io::AsyncWriteRent,
    BufResult,
};

/// AsyncWriteRentExt
pub trait AsyncWriteRentExt {
    /// The future of Result<size, buffer>
    type WriteExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoBuf + 'a;

    /// Write all
    fn write_all<T>(&mut self, buf: T) -> Self::WriteExactFuture<'_, T>
    where
        T: 'static + IoBuf;

    /// The future of Result<size, buffer>
    type WriteVectoredExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoVecBuf + 'a;

    /// Write all
    fn write_vectored_all<T>(&mut self, buf: T) -> Self::WriteVectoredExactFuture<'_, T>
    where
        T: 'static + IoVecBuf;
}

impl<A> AsyncWriteRentExt for A
where
    A: AsyncWriteRent + ?Sized,
{
    type WriteExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> + 'a where A: 'a, T: IoBuf + 'a;

    fn write_all<T>(&mut self, mut buf: T) -> Self::WriteExactFuture<'_, T>
    where
        T: 'static + IoBuf,
    {
        async move {
            let len = buf.bytes_init();
            let mut written = 0;
            while written < len {
                let buf_slice = unsafe { Slice::new_unchecked(buf, written, len) };
                let (result, buf_slice) = self.write(buf_slice).await;
                buf = buf_slice.into_inner();
                match result {
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

    type WriteVectoredExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> + 'a where A: 'a, T: IoVecBuf + 'a;

    fn write_vectored_all<T>(&mut self, buf: T) -> Self::WriteVectoredExactFuture<'_, T>
    where
        T: 'static + IoVecBuf,
    {
        let mut meta = crate::buf::read_vec_meta(&buf);
        let len = meta.len();
        let mut written = 0;

        async move {
            while written < len {
                let (res, meta_) = self.writev(meta).await;
                meta = meta_;
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
                    Ok(n) => {
                        written += n;
                        meta.consume(n);
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => return (Err(e), buf),
                }
            }
            (Ok(written), buf)
        }
    }
}
