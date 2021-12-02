use crate::buf::{IoBuf, IoBufMut};

use std::ops;

pub struct SliceMut<T> {
    buf: T,
    begin: usize,
    end: usize,
}

impl<T> SliceMut<T> {
    pub(crate) fn new(buf: T, begin: usize, end: usize) -> SliceMut<T> {
        SliceMut {
            buf,
            begin,
            end,
        }
    }

    /// Offset in the underlying buffer at which this slice starts.
    #[inline]
    pub fn begin(&self) -> usize {
        self.begin
    }

    #[inline]
    pub fn end(&self) -> usize {
        self.end
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.buf
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.buf
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.buf
    }
}

impl<T: IoBuf> ops::Deref for SliceMut<T> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &super::deref(&self.buf)[self.begin..self.end]
    }
}

impl<T: IoBufMut> ops::DerefMut for SliceMut<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut super::deref_mut(&mut self.buf)[self.begin..self.end]
    }
}

unsafe impl<T: IoBuf> IoBuf for SliceMut<T> {
    #[inline]
    fn stable_ptr(&self) -> *const u8 {
        ops::Deref::deref(self).as_ptr()
    }

    #[inline]
    fn len(&self) -> usize {
        self.end - self.begin
    }
}

unsafe impl<T: IoBufMut> IoBufMut for SliceMut<T> {
    #[inline]
    fn stable_mut_ptr(&mut self) -> *mut u8 {
        ops::DerefMut::deref_mut(self).as_mut_ptr()
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.end - self.begin
    }

    #[inline]
    unsafe fn set_init(&mut self, pos: usize) {
        self.buf.set_init(self.begin + pos);
    }
}
