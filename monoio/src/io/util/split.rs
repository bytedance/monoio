use std::{cell::UnsafeCell, rc::Rc};

use crate::io::{AsyncReadRent, AsyncWriteRent};

pub struct OwnedReadHalf<T>(Rc<UnsafeCell<T>>);
pub struct OwnedWriteHalf<T>(Rc<UnsafeCell<T>>);
pub struct WriteHalf<'cx, T>(&'cx T);
pub struct ReadHalf<'cx, T>(&'cx T);

/// This is a dummy unsafe trait to inform monoio,
/// the object with has this `Split` trait can be safely split
/// to read/write object in both form of `Owned` or `Borrowed`.
/// Note: monoio cannot guarantee whether the custom object can be
/// safely aplit to divided objects. Users should ensure the safety
/// by themselves.
pub unsafe trait Split {}

/// Inner split trait
pub trait _Split {
    /// Owned Read Split
    type OwnedRead;
    /// Owned Write Split
    type OwnedWrite;

    /// Borrowed Read Split
    type Read<'cx>
    where
        Self: 'cx;
    /// Borrowed Write Split
    type Write<'cx>
    where
        Self: 'cx;

    /// Split into owned parts
    fn into_split(self) -> (Self::OwnedRead, Self::OwnedWrite);

    /// Split into borrowed parts
    fn split(&self) -> (Self::Read<'_>, Self::Write<'_>);
}

impl<T> _Split for T
where
    T: Split + AsyncReadRent + AsyncWriteRent,
{
    type Read<'cx> = ReadHalf<'cx, T> where Self: 'cx;

    type Write<'cx> = WriteHalf<'cx, T> where Self: 'cx;

    type OwnedRead = OwnedReadHalf<T>;

    type OwnedWrite = OwnedWriteHalf<T>;

    fn into_split(self) -> (Self::OwnedRead, Self::OwnedWrite) {
        let shared = Rc::new(UnsafeCell::new(self));
        (OwnedReadHalf(shared.clone()), OwnedWriteHalf(shared))
    }

    fn split(&self) -> (Self::Read<'_>, Self::Write<'_>) {
        (ReadHalf(&*self), WriteHalf(&*self))
    }
}
