use std::ops::{Deref, DerefMut};

#[cfg(unix)]
use libc::msghdr;
#[cfg(windows)]
use windows_sys::Win32::Networking::WinSock::WSAMSG;

/// An `io_uring` compatible msg buffer.
///
/// # Safety
/// See the safety note of the methods.
#[allow(clippy::unnecessary_safety_doc)]
pub unsafe trait MsgBuf: Unpin + 'static {
    /// Returns a raw pointer to msghdr struct.
    ///
    /// # Safety
    /// The implementation must ensure that, while the runtime owns the value,
    /// the pointer returned by `stable_mut_ptr` **does not** change.
    /// Also, the value pointed must be a valid msghdr struct.
    #[cfg(unix)]
    fn read_msghdr_ptr(&self) -> *const msghdr;

    /// Returns a raw pointer to WSAMSG struct.
    #[cfg(windows)]
    fn read_wsamsg_ptr(&self) -> *const WSAMSG;
}

/// An `io_uring` compatible msg buffer.
///
/// # Safety
/// See the safety note of the methods.
#[allow(clippy::unnecessary_safety_doc)]
pub unsafe trait MsgBufMut: Unpin + 'static {
    /// Returns a raw pointer to msghdr struct.
    ///
    /// # Safety
    /// The implementation must ensure that, while the runtime owns the value,
    /// the pointer returned by `stable_mut_ptr` **does not** change.
    /// Also, the value pointed must be a valid msghdr struct.
    #[cfg(unix)]
    fn write_msghdr_ptr(&mut self) -> *mut msghdr;

    /// Returns a raw pointer to WSAMSG struct.
    #[cfg(windows)]
    fn write_wsamsg_ptr(&mut self) -> *mut WSAMSG;
}

#[allow(missing_docs)]
pub struct MsgMeta {
    #[cfg(unix)]
    pub(crate) data: msghdr,
    #[cfg(windows)]
    pub(crate) data: WSAMSG,
}

unsafe impl MsgBuf for MsgMeta {
    #[cfg(unix)]
    fn read_msghdr_ptr(&self) -> *const msghdr {
        &self.data
    }

    #[cfg(windows)]
    fn read_wsamsg_ptr(&self) -> *const WSAMSG {
        &self.data
    }
}

unsafe impl MsgBufMut for MsgMeta {
    #[cfg(unix)]
    fn write_msghdr_ptr(&mut self) -> *mut msghdr {
        &mut self.data
    }

    #[cfg(windows)]
    fn write_wsamsg_ptr(&mut self) -> *mut WSAMSG {
        &mut self.data
    }
}

#[cfg(unix)]
impl From<msghdr> for MsgMeta {
    fn from(data: msghdr) -> Self {
        Self { data }
    }
}

#[cfg(unix)]
impl Deref for MsgMeta {
    type Target = msghdr;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(unix)]
impl DerefMut for MsgMeta {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[cfg(windows)]
impl From<WSAMSG> for MsgMeta {
    fn from(data: WSAMSG) -> Self {
        Self { data }
    }
}

#[cfg(windows)]
impl Deref for MsgMeta {
    type Target = WSAMSG;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(windows)]
impl DerefMut for MsgMeta {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}
