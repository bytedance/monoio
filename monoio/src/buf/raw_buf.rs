use std::ptr::null;

use super::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut};

/// RawBuf is not a real buf. It only hold the pointer of the buffer.
/// Users must make sure the buffer behind the pointer is always valid.
/// Which means, user must:
/// 1. await the future with RawBuf Ready before drop the real buffer
/// 2. make sure the pointer and length is valid before the future Ready
pub struct RawBuf {
    ptr: *const u8,
    len: usize,
}

impl RawBuf {
    /// Create a empty RawBuf.
    /// # Safety
    /// do not use uninitialized RawBuf directly.
    pub unsafe fn uninit() -> Self {
        Self {
            ptr: null(),
            len: 0,
        }
    }

    /// Create a new RawBuf with given pointer and length.
    /// # Safety
    /// make sure the pointer and length is valid when RawBuf is used.
    pub unsafe fn new(ptr: *const u8, len: usize) -> Self {
        Self { ptr, len }
    }
}

unsafe impl IoBuf for RawBuf {
    fn read_ptr(&self) -> *const u8 {
        self.ptr
    }

    fn bytes_init(&self) -> usize {
        self.len
    }
}

unsafe impl IoBufMut for RawBuf {
    fn write_ptr(&mut self) -> *mut u8 {
        self.ptr as *mut u8
    }

    fn bytes_total(&self) -> usize {
        self.len
    }

    unsafe fn set_init(&mut self, _pos: usize) {}
}

impl RawBuf {
    /// Create a new RawBuf with the first iovec part.
    /// # Safety
    /// make sure the pointer and length is valid when RawBuf is used.
    pub unsafe fn new_from_iovec_mut<T: IoVecBufMut>(data: &mut T) -> Option<Self> {
        if data.write_iovec_len() == 0 {
            return None;
        }
        let iovec = *data.write_iovec_ptr();
        Some(Self::new(iovec.iov_base as *const u8, iovec.iov_len))
    }

    /// Create a new RawBuf with the first iovec part.
    /// # Safety
    /// make sure the pointer and length is valid when RawBuf is used.
    pub unsafe fn new_from_iovec<T: IoVecBuf>(data: &T) -> Option<Self> {
        if data.read_iovec_len() == 0 {
            return None;
        }
        let iovec = *data.read_iovec_ptr();
        Some(Self::new(iovec.iov_base as *const u8, iovec.iov_len))
    }
}
