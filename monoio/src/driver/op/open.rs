use super::{Op, OpAble};
use crate::driver::legacy::ready::Direction;
use crate::driver::util::cstr;
use crate::fs::OpenOptions;
use crate::syscall_u32;

#[cfg(target_os = "linux")]
use io_uring::{opcode, types};

use std::ffi::CString;
use std::io;
use std::path::Path;

/// Open a file
pub(crate) struct Open {
    pub(crate) path: CString,
    flags: i32,
    mode: libc::mode_t,
}

impl Op<Open> {
    /// Submit a request to open a file.
    pub(crate) fn open<P: AsRef<Path>>(path: P, options: &OpenOptions) -> io::Result<Op<Open>> {
        // Here the path will be copied, so its safe.
        let path = cstr(path.as_ref())?;
        let flags = libc::O_CLOEXEC | options.access_mode()? | options.creation_mode()?;
        let mode = options.mode;

        Op::submit_with(Open { path, flags, mode })
    }
}

impl OpAble for Open {
    #[cfg(target_os = "linux")]
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::OpenAt::new(types::Fd(libc::AT_FDCWD), self.path.as_c_str().as_ptr())
            .flags(self.flags)
            .mode(self.mode)
            .build()
    }

    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    fn legacy_call(self: &mut std::pin::Pin<Box<Self>>) -> io::Result<u32> {
        syscall_u32!(open(
            self.path.as_c_str().as_ptr(),
            self.flags,
            self.mode as libc::c_int
        ))
    }
}
