use super::AsyncReadRent;
use crate::{
    buf::{IoBufMut, IoVecBufMut, RawBuf},
    BufResult,
};
use std::future::Future;

macro_rules! reader_trait {
    ($future: ident, $n_ty: ty, $f: ident) => {
        /// Read number result
        type $future<'a>: Future<Output = std::io::Result<$n_ty>>
        where
            Self: 'a;

        /// Read number in async way
        fn $f(&mut self) -> Self::$future<'_>;
    };
}

macro_rules! reader_be_impl {
    ($future: ident, $n_ty: ty, $f: ident) => {
        type $future<'a> = impl Future<Output = std::io::Result<$n_ty>> where A: 'a;

        fn $f(&mut self) -> Self::$future<'_> {
            async {
                let (res, buf) = self.read_exact([0; std::mem::size_of::<$n_ty>()]).await;
                res?;
                Ok(<$n_ty>::from_be_bytes(buf))
            }
        }
    };
}

macro_rules! reader_le_impl {
    ($future: ident, $n_ty: ty, $f: ident) => {
        type $future<'a> = impl Future<Output = std::io::Result<$n_ty>> where A: 'a;

        fn $f(&mut self) -> Self::$future<'_> {
            async {
                let (res, buf) = self.read_exact([0; std::mem::size_of::<$n_ty>()]).await;
                res?;
                Ok(<$n_ty>::from_le_bytes(buf))
            }
        }
    };
}

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

    reader_trait!(ReadU8Future, u8, read_u8);
    reader_trait!(ReadU16Future, u16, read_u16);
    reader_trait!(ReadU32Future, u32, read_u32);
    reader_trait!(ReadU64Future, u64, read_u64);
    reader_trait!(ReadU128Future, u16, read_u128);
    reader_trait!(ReadI8Future, i8, read_i8);
    reader_trait!(ReadI16Future, i16, read_i16);
    reader_trait!(ReadI32Future, i32, read_i32);
    reader_trait!(ReadI64Future, i64, read_i64);
    reader_trait!(ReadI128Future, i128, read_i128);
    reader_trait!(ReadF32Future, f32, read_f32);
    reader_trait!(ReadF64Future, f64, read_f64);

    reader_trait!(ReadU8LEFuture, u8, read_u8_le);
    reader_trait!(ReadU16LEFuture, u16, read_u16_le);
    reader_trait!(ReadU32LEFuture, u32, read_u32_le);
    reader_trait!(ReadU64LEFuture, u64, read_u64_le);
    reader_trait!(ReadU128LEFuture, u16, read_u128_le);
    reader_trait!(ReadI8LEFuture, i8, read_i8_le);
    reader_trait!(ReadI16LEFuture, i16, read_i16_le);
    reader_trait!(ReadI32LEFuture, i32, read_i32_le);
    reader_trait!(ReadI64LEFuture, i64, read_i64_le);
    reader_trait!(ReadI128LEFuture, i128, read_i128_le);
    reader_trait!(ReadF32LEFuture, f32, read_f32_le);
    reader_trait!(ReadF64LEFuture, f64, read_f64_le);
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
            let ptr = buf.write_ptr();
            let len = buf.bytes_total();
            let mut read = 0;
            while read < len {
                let raw_buf = unsafe { RawBuf::new(ptr.add(read), len - read) };
                match self.read(raw_buf).await.0 {
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

    reader_be_impl!(ReadU8Future, u8, read_u8);
    reader_be_impl!(ReadU16Future, u16, read_u16);
    reader_be_impl!(ReadU32Future, u32, read_u32);
    reader_be_impl!(ReadU64Future, u64, read_u64);
    reader_be_impl!(ReadU128Future, u16, read_u128);
    reader_be_impl!(ReadI8Future, i8, read_i8);
    reader_be_impl!(ReadI16Future, i16, read_i16);
    reader_be_impl!(ReadI32Future, i32, read_i32);
    reader_be_impl!(ReadI64Future, i64, read_i64);
    reader_be_impl!(ReadI128Future, i128, read_i128);
    reader_be_impl!(ReadF32Future, f32, read_f32);
    reader_be_impl!(ReadF64Future, f64, read_f64);

    reader_le_impl!(ReadU8LEFuture, u8, read_u8_le);
    reader_le_impl!(ReadU16LEFuture, u16, read_u16_le);
    reader_le_impl!(ReadU32LEFuture, u32, read_u32_le);
    reader_le_impl!(ReadU64LEFuture, u64, read_u64_le);
    reader_le_impl!(ReadU128LEFuture, u16, read_u128_le);
    reader_le_impl!(ReadI8LEFuture, i8, read_i8_le);
    reader_le_impl!(ReadI16LEFuture, i16, read_i16_le);
    reader_le_impl!(ReadI32LEFuture, i32, read_i32_le);
    reader_le_impl!(ReadI64LEFuture, i64, read_i64_le);
    reader_le_impl!(ReadI128LEFuture, i128, read_i128_le);
    reader_be_impl!(ReadF32LEFuture, f32, read_f32_le);
    reader_be_impl!(ReadF64LEFuture, f64, read_f64_le);
}
