//! Utilities for working with buffers.
//!
//! `io-uring` APIs require passing ownership of buffers to the runtime. The
//! crate defines [`IoBuf`] and [`IoBufMut`] traits which are implemented by buffer
//! types that respect the `io-uring` contract.
// Heavily borrowed from tokio-uring.
// Copyright (c) 2021 Tokio-uring Contributors, licensed under the MIT license.

mod io_buf;
pub use io_buf::{IoBuf, IoBufMut};

mod io_vec_buf;
pub use io_vec_buf::{IoVecBuf, IoVecBufMut, VecBuf};

mod slice;
pub use slice::Slice;

mod shared_buf;
pub use shared_buf::{Shared, SharedBuf};

pub(crate) fn deref(buf: &impl IoBuf) -> &[u8] {
    // Safety: the `IoBuf` trait is marked as unsafe and is expected to be
    // implemented correctly.
    unsafe { std::slice::from_raw_parts(buf.stable_ptr(), buf.bytes_init()) }
}

pub(crate) fn deref_mut(buf: &mut impl IoBufMut) -> &mut [u8] {
    // Safety: the `IoBufMut` trait is marked as unsafe and is expected to be
    // implemented correct.
    unsafe { std::slice::from_raw_parts_mut(buf.stable_mut_ptr(), buf.bytes_init()) }
}
