use std::future::Future;

use super::{CancelHandle, CancelableAsyncReadRent, CancelableAsyncWriteRent};
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut, Slice, SliceMut},
    BufResult,
};

macro_rules! reader_trait {
    ($future: ident, $n_ty: ty, $f: ident) => {
        /// Read number in async way
        fn $f(&mut self, c: CancelHandle) -> impl Future<Output = std::io::Result<$n_ty>>;
    };
}

macro_rules! reader_be_impl {
    ($future: ident, $n_ty: ty, $f: ident) => {
        async fn $f(&mut self, c: CancelHandle) -> std::io::Result<$n_ty> {
            let (res, buf) = self
                .cancelable_read_exact(std::boxed::Box::new([0; std::mem::size_of::<$n_ty>()]), c)
                .await;
            res?;
            use crate::utils::box_into_inner::IntoInner;
            Ok(<$n_ty>::from_be_bytes(Box::consume(buf)))
        }
    };
}

macro_rules! reader_le_impl {
    ($future: ident, $n_ty: ty, $f: ident) => {
        async fn $f(&mut self, c: CancelHandle) -> std::io::Result<$n_ty> {
            let (res, buf) = self
                .cancelable_read_exact(std::boxed::Box::new([0; std::mem::size_of::<$n_ty>()]), c)
                .await;
            res?;
            use crate::utils::box_into_inner::IntoInner;
            Ok(<$n_ty>::from_le_bytes(Box::consume(buf)))
        }
    };
}

/// CancelableAsyncReadRentExt
pub trait CancelableAsyncReadRentExt {
    /// Read until buf capacity is fulfilled
    fn cancelable_read_exact<T: IoBufMut + 'static>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>>;

    /// Readv until buf capacity is fulfilled
    fn cancelable_read_vectored_exact<T: IoVecBufMut + 'static>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>>;

    reader_trait!(ReadU8Future, u8, cancelable_read_u8);
    reader_trait!(ReadU16Future, u16, cancelable_read_u16);
    reader_trait!(ReadU32Future, u32, cancelable_read_u32);
    reader_trait!(ReadU64Future, u64, cancelable_read_u64);
    reader_trait!(ReadU128Future, u128, cancelable_read_u128);
    reader_trait!(ReadI8Future, i8, cancelable_read_i8);
    reader_trait!(ReadI16Future, i16, cancelable_read_i16);
    reader_trait!(ReadI32Future, i32, cancelable_read_i32);
    reader_trait!(ReadI64Future, i64, cancelable_read_i64);
    reader_trait!(ReadI128Future, i128, cancelable_read_i128);
    reader_trait!(ReadF32Future, f32, cancelable_read_f32);
    reader_trait!(ReadF64Future, f64, cancelable_read_f64);

    reader_trait!(ReadU8LEFuture, u8, cancelable_read_u8_le);
    reader_trait!(ReadU16LEFuture, u16, cancelable_read_u16_le);
    reader_trait!(ReadU32LEFuture, u32, cancelable_read_u32_le);
    reader_trait!(ReadU64LEFuture, u64, cancelable_read_u64_le);
    reader_trait!(ReadU128LEFuture, u128, cancelable_read_u128_le);
    reader_trait!(ReadI8LEFuture, i8, cancelable_read_i8_le);
    reader_trait!(ReadI16LEFuture, i16, cancelable_read_i16_le);
    reader_trait!(ReadI32LEFuture, i32, cancelable_read_i32_le);
    reader_trait!(ReadI64LEFuture, i64, cancelable_read_i64_le);
    reader_trait!(ReadI128LEFuture, i128, cancelable_read_i128_le);
    reader_trait!(ReadF32LEFuture, f32, cancelable_read_f32_le);
    reader_trait!(ReadF64LEFuture, f64, cancelable_read_f64_le);
}

impl<A> CancelableAsyncReadRentExt for A
where
    A: CancelableAsyncReadRent + ?Sized,
{
    async fn cancelable_read_exact<T: IoBufMut + 'static>(
        &mut self,
        mut buf: T,
        c: CancelHandle,
    ) -> BufResult<usize, T> {
        let len = buf.bytes_total();
        let mut read = 0;
        while read < len {
            let buf_slice = unsafe { SliceMut::new_unchecked(buf, read, len) };
            let (result, buf_slice) = self.cancelable_read(buf_slice, c.clone()).await;
            buf = buf_slice.into_inner();
            match result {
                Ok(0) => {
                    return (
                        Err(std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "failed to fill whole buffer",
                        )),
                        buf,
                    )
                }
                Ok(n) => {
                    read += n;
                    unsafe { buf.set_init(read) };
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return (Err(e), buf),
            }
        }
        (Ok(read), buf)
    }

    async fn cancelable_read_vectored_exact<T: IoVecBufMut + 'static>(
        &mut self,
        mut buf: T,
        c: CancelHandle,
    ) -> BufResult<usize, T> {
        let mut meta = crate::buf::write_vec_meta(&mut buf);
        let len = meta.len();
        let mut read = 0;

        while read < len {
            let (res, meta_) = self.cancelable_readv(meta, c.clone()).await;
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

    reader_be_impl!(ReadU8Future, u8, cancelable_read_u8);
    reader_be_impl!(ReadU16Future, u16, cancelable_read_u16);
    reader_be_impl!(ReadU32Future, u32, cancelable_read_u32);
    reader_be_impl!(ReadU64Future, u64, cancelable_read_u64);
    reader_be_impl!(ReadU128Future, u128, cancelable_read_u128);
    reader_be_impl!(ReadI8Future, i8, cancelable_read_i8);
    reader_be_impl!(ReadI16Future, i16, cancelable_read_i16);
    reader_be_impl!(ReadI32Future, i32, cancelable_read_i32);
    reader_be_impl!(ReadI64Future, i64, cancelable_read_i64);
    reader_be_impl!(ReadI128Future, i128, cancelable_read_i128);
    reader_be_impl!(ReadF32Future, f32, cancelable_read_f32);
    reader_be_impl!(ReadF64Future, f64, cancelable_read_f64);

    reader_le_impl!(ReadU8LEFuture, u8, cancelable_read_u8_le);
    reader_le_impl!(ReadU16LEFuture, u16, cancelable_read_u16_le);
    reader_le_impl!(ReadU32LEFuture, u32, cancelable_read_u32_le);
    reader_le_impl!(ReadU64LEFuture, u64, cancelable_read_u64_le);
    reader_le_impl!(ReadU128LEFuture, u128, cancelable_read_u128_le);
    reader_le_impl!(ReadI8LEFuture, i8, cancelable_read_i8_le);
    reader_le_impl!(ReadI16LEFuture, i16, cancelable_read_i16_le);
    reader_le_impl!(ReadI32LEFuture, i32, cancelable_read_i32_le);
    reader_le_impl!(ReadI64LEFuture, i64, cancelable_read_i64_le);
    reader_le_impl!(ReadI128LEFuture, i128, cancelable_read_i128_le);
    reader_be_impl!(ReadF32LEFuture, f32, cancelable_read_f32_le);
    reader_be_impl!(ReadF64LEFuture, f64, cancelable_read_f64_le);
}

/// CancelableAsyncWriteRentExt
pub trait CancelableAsyncWriteRentExt {
    /// Write all
    fn write_all<T: IoBuf + 'static>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>>;

    /// Write all
    fn write_vectored_all<T: IoVecBuf + 'static>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> impl Future<Output = BufResult<usize, T>>;
}

impl<A> CancelableAsyncWriteRentExt for A
where
    A: CancelableAsyncWriteRent + ?Sized,
{
    async fn write_all<T: IoBuf + 'static>(
        &mut self,
        mut buf: T,
        c: CancelHandle,
    ) -> BufResult<usize, T> {
        let len = buf.bytes_init();
        let mut written = 0;
        while written < len {
            let buf_slice = unsafe { Slice::new_unchecked(buf, written, len) };
            let (result, buf_slice) = self.cancelable_write(buf_slice, c.clone()).await;
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

    async fn write_vectored_all<T: IoVecBuf + 'static>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> BufResult<usize, T> {
        let mut meta = crate::buf::read_vec_meta(&buf);
        let len = meta.len();
        let mut written = 0;

        while written < len {
            let (res, meta_) = self.cancelable_writev(meta, c.clone()).await;
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
