//! We impl AsReadFd and AsWriteFd for some structs.

use crate::driver::shared_fd::SharedFd;

/// Get a readable shared fd from self.
pub trait AsReadFd {
    /// Get fd.
    fn as_reader_fd(&mut self) -> &SharedFdWrapper;
}

/// Get a writable shared fd from self.
pub trait AsWriteFd {
    /// Get fd.
    fn as_writer_fd(&mut self) -> &SharedFdWrapper;
}

/// A wrapper of SharedFd to solve pub problem.
#[repr(transparent)]
pub struct SharedFdWrapper(SharedFd);

impl SharedFdWrapper {
    #[allow(unused)]
    pub(crate) fn as_ref(&self) -> &SharedFd {
        &self.0
    }

    pub(crate) fn new(inner: &SharedFd) -> &Self {
        unsafe { std::mem::transmute(inner) }
    }
}
