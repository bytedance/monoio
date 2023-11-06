use std::future::Future;

use crate::{
    buf::{IoBuf, IoVecBuf, Slice},
    io::AsyncWriteRent,
    BufResult,
};

/// AsyncWriteRentExt
pub trait AsyncWriteRentExt {
    /// Write all
    fn write_all<T: IoBuf + 'static>(
        &mut self,
        buf: T,
    ) -> impl Future<Output = BufResult<usize, T>>;

    /// Write vectored all
    fn write_vectored_all<T: IoVecBuf + 'static>(
        &mut self,
        buf: T,
    ) -> impl Future<Output = BufResult<usize, T>>;
}

impl<A> AsyncWriteRentExt for A
where
    A: AsyncWriteRent + ?Sized,
{
    async fn write_all<T: IoBuf + 'static>(&mut self, mut buf: T) -> BufResult<usize, T> {
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

    async fn write_vectored_all<T: IoVecBuf + 'static>(&mut self, buf: T) -> BufResult<usize, T> {
        let mut meta = crate::buf::read_vec_meta(&buf);
        let len = meta.len();
        let mut written = 0;

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
