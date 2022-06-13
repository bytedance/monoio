use crate::{
    buf::{IoBuf, IoVecBuf, RawBuf},
    io::AsyncWriteRent,
    BufResult,
};
use std::future::Future;

/// AsyncWriteRentExt
pub trait AsyncWriteRentExt {
    /// The future of Result<size, buffer>
    type WriteExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Write all
    fn write_all<T>(&mut self, buf: T) -> Self::WriteExactFuture<'_, T>
    where
        T: 'static + IoBuf;

    /// The future of Result<size, buffer>
    type WriteVectoredExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: 'a;

    /// Write all
    fn write_vectored_all<T>(&mut self, buf: T) -> Self::WriteVectoredExactFuture<'_, T>
    where
        T: 'static + IoVecBuf;
}

impl<A> AsyncWriteRentExt for A
where
    A: AsyncWriteRent + ?Sized,
{
    type WriteExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> where A: 'a, T: 'a;

    fn write_all<T>(&mut self, buf: T) -> Self::WriteExactFuture<'_, T>
    where
        T: 'static + IoBuf,
    {
        async move {
            let ptr = buf.read_ptr();
            let len = buf.bytes_init();
            let mut written = 0;
            while written < len {
                let raw_buf = unsafe { RawBuf::new(ptr.add(written), len - written) };
                match self.write(raw_buf).await.0 {
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

    type WriteVectoredExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> where A: 'a, T: 'a;

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
