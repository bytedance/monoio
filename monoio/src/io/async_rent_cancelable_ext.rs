use std::future::Future;

use super::{CancelHandle, CancelableAsyncReadRent, CancelableAsyncWriteRent};
use crate::{
    buf::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut, Slice, SliceMut},
    BufResult,
};

macro_rules! reader_trait {
    ($future: ident, $n_ty: ty, $f: ident) => {
        /// Read number result
        type $future<'a>: Future<Output = std::io::Result<$n_ty>>
        where
            Self: 'a;

        /// Read number in async way
        fn $f(&mut self, c: CancelHandle) -> Self::$future<'_>;
    };
}

macro_rules! reader_be_impl {
    ($future: ident, $n_ty: ty, $f: ident) => {
        type $future<'a> = impl Future<Output = std::io::Result<$n_ty>> + 'a where A: 'a;

        fn $f(&mut self, c: CancelHandle) -> Self::$future<'_> {
            async {
                let (res, buf) = self
                    .cancelable_read_exact(
                        std::boxed::Box::new([0; std::mem::size_of::<$n_ty>()]),
                        c,
                    )
                    .await;
                res?;
                use crate::utils::box_into_inner::IntoInner;
                Ok(<$n_ty>::from_be_bytes(Box::consume(buf)))
            }
        }
    };
}

macro_rules! reader_le_impl {
    ($future: ident, $n_ty: ty, $f: ident) => {
        type $future<'a> = impl Future<Output = std::io::Result<$n_ty>> + 'a where A: 'a;

        fn $f(&mut self, c: CancelHandle) -> Self::$future<'_> {
            async {
                let (res, buf) = self
                    .cancelable_read_exact(
                        std::boxed::Box::new([0; std::mem::size_of::<$n_ty>()]),
                        c,
                    )
                    .await;
                res?;
                use crate::utils::box_into_inner::IntoInner;
                Ok(<$n_ty>::from_le_bytes(Box::consume(buf)))
            }
        }
    };
}

/// CancelableAsyncReadRentExt
pub trait CancelableAsyncReadRentExt {
    /// The future of Result<size, buffer>
    type CancelableReadExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoBufMut + 'a;

    /// Read until buf capacity is fulfilled
    fn cancelable_read_exact<T: 'static>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadExactFuture<'_, T>
    where
        T: 'static + IoBufMut;

    /// The future of Result<size, buffer>
    type CancelableReadVectoredExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoVecBufMut + 'a;

    /// Readv until buf capacity is fulfilled
    fn cancelable_read_vectored_exact<T: 'static>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadVectoredExactFuture<'_, T>
    where
        T: 'static + IoVecBufMut;

    reader_trait!(ReadU8Future, u8, cancelable_read_u8);
    reader_trait!(ReadU16Future, u16, cancelable_read_u16);
    reader_trait!(ReadU32Future, u32, cancelable_read_u32);
    reader_trait!(ReadU64Future, u64, cancelable_read_u64);
    reader_trait!(ReadU128Future, u16, cancelable_read_u128);
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
    reader_trait!(ReadU128LEFuture, u16, cancelable_read_u128_le);
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
    type CancelableReadExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> + 'a where A: 'a, T: IoBufMut + 'a;

    fn cancelable_read_exact<T>(
        &mut self,
        mut buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadExactFuture<'_, T>
    where
        T: 'static + IoBufMut,
    {
        async move {
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
    }

    type CancelableReadVectoredExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> + 'a where A: 'a, T: IoVecBufMut + 'a;

    fn cancelable_read_vectored_exact<T: 'static>(
        &mut self,
        mut buf: T,
        c: CancelHandle,
    ) -> Self::CancelableReadVectoredExactFuture<'_, T>
    where
        T: 'static + IoVecBufMut,
    {
        let mut meta = crate::buf::write_vec_meta(&mut buf);
        let len = meta.len();
        let mut read = 0;
        async move {
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
    }

    reader_be_impl!(ReadU8Future, u8, cancelable_read_u8);
    reader_be_impl!(ReadU16Future, u16, cancelable_read_u16);
    reader_be_impl!(ReadU32Future, u32, cancelable_read_u32);
    reader_be_impl!(ReadU64Future, u64, cancelable_read_u64);
    reader_be_impl!(ReadU128Future, u16, cancelable_read_u128);
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
    reader_le_impl!(ReadU128LEFuture, u16, cancelable_read_u128_le);
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
    /// The future of Result<size, buffer>
    type WriteExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoBuf + 'a;

    /// Write all
    fn write_all<T>(&mut self, buf: T, c: CancelHandle) -> Self::WriteExactFuture<'_, T>
    where
        T: 'static + IoBuf;

    /// The future of Result<size, buffer>
    type WriteVectoredExactFuture<'a, T>: Future<Output = BufResult<usize, T>>
    where
        Self: 'a,
        T: IoVecBuf + 'a;

    /// Write all
    fn write_vectored_all<T>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::WriteVectoredExactFuture<'_, T>
    where
        T: 'static + IoVecBuf;
}

impl<A> CancelableAsyncWriteRentExt for A
where
    A: CancelableAsyncWriteRent + ?Sized,
{
    type WriteExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> + 'a where A: 'a, T: IoBuf + 'a;

    fn write_all<T>(&mut self, mut buf: T, c: CancelHandle) -> Self::WriteExactFuture<'_, T>
    where
        T: 'static + IoBuf,
    {
        async move {
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
    }

    type WriteVectoredExactFuture<'a, T> = impl Future<Output = BufResult<usize, T>> + 'a where A: 'a, T: IoVecBuf + 'a;

    fn write_vectored_all<T>(
        &mut self,
        buf: T,
        c: CancelHandle,
    ) -> Self::WriteVectoredExactFuture<'_, T>
    where
        T: 'static + IoVecBuf,
    {
        let mut meta = crate::buf::read_vec_meta(&buf);
        let len = meta.len();
        let mut written = 0;

        async move {
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
}
