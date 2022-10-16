use std::{mem::MaybeUninit, ptr::null};

use monoio::buf::{IoBuf, IoBufMut};

/// RawBuf is not a real buf. It only hold the pointer of the buffer.
/// Users must make sure the buffer behind the pointer is always valid.
/// Which means, user must:
/// 1. await the future with RawBuf Ready before drop the real buffer
/// 2. make sure the pointer and length is valid before the future Ready
pub(crate) struct RawBuf {
    ptr: *const u8,
    len: usize,
}

impl RawBuf {
    pub(crate) fn uninit() -> Self {
        Self {
            ptr: null(),
            len: 0,
        }
    }

    pub(crate) fn new(ptr: *const u8, len: usize) -> Self {
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

    fn bytes_total(&mut self) -> usize {
        self.len
    }

    unsafe fn set_init(&mut self, _pos: usize) {}
}

/// Buf is a real buffer with data. It is used by "safe" TcpStreamCompat.
/// Read: If there is some data inside buf, copy it directly and return.
///       Otherwise, do async read and save the future. When the future
///       finished, init and offset will be reset.
/// Write: Copy data into our buffer if we not hold it. After that, we
///        will create future, save it and poll it. If we hold data,
///        we may check the buffer ptr and length(because user must make
///        sure it's the same slice). The saved future will be polled.
pub(crate) struct Buf {
    data: Box<[MaybeUninit<u8>]>,
    offset: usize,
    init: usize,
    capacity: usize,
}

unsafe impl IoBuf for Buf {
    fn read_ptr(&self) -> *const u8 {
        unsafe { self.data.as_ptr().add(self.offset) as *const u8 }
    }

    fn bytes_init(&self) -> usize {
        self.init - self.offset
    }
}

unsafe impl IoBufMut for Buf {
    fn write_ptr(&mut self) -> *mut u8 {
        self.data.as_ptr() as *mut u8
    }

    fn bytes_total(&mut self) -> usize {
        self.capacity
    }

    unsafe fn set_init(&mut self, init: usize) {
        self.offset = 0;
        self.init = init;
    }
}

impl Buf {
    pub(crate) fn new(size: usize) -> Self {
        let mut buf = Vec::with_capacity(size);
        unsafe { buf.set_len(size) };
        let data = buf.into_boxed_slice();
        Self {
            data,
            offset: 0,
            init: 0,
            capacity: size,
        }
    }

    pub(crate) fn uninit() -> Self {
        let buf = Vec::new();
        let data = buf.into_boxed_slice();
        Self {
            data,
            offset: 0,
            init: 0,
            capacity: 0,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.offset == self.init
    }

    /// Return slice for copying data from Buf to user space.
    pub(crate) fn buf_to_read(&self, max: usize) -> &[u8] {
        let ptr = self.data.as_ptr() as *const u8;
        let len = max.min(self.init - self.offset);
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    /// Advance offset.
    /// # Safety: User must ensure the cursor position after advancing is initialized.
    pub(crate) unsafe fn advance_offset(&mut self, len: usize) {
        self.offset += len;
        debug_assert!(self.offset <= self.init);
    }

    /// Return slice for copying data from user space to Buf.
    #[allow(clippy::mut_from_ref)]
    pub(crate) fn buf_to_write(&mut self) -> &mut [u8] {
        let ptr = self.data.as_ptr() as *mut u8;
        unsafe { std::slice::from_raw_parts_mut(ptr, self.capacity) }
    }
}
