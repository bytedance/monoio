use std::{ffi::CString, io, path::Path};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};

use super::{Op, OpAble};
#[cfg(all(unix, feature = "legacy"))]
use crate::{driver::legacy::ready::Direction, syscall_u32};
use crate::{driver::util::cstr, fs::OpenOptions};

/// Open a file
pub(crate) struct Open {
    pub(crate) path: CString,
    flags: i32,
    #[cfg(unix)]
    mode: libc::mode_t,
}

impl Op<Open> {
    #[cfg(unix)]
    /// Submit a request to open a file.
    pub(crate) fn open<P: AsRef<Path>>(path: P, options: &OpenOptions) -> io::Result<Op<Open>> {
        // Here the path will be copied, so its safe.
        let path = cstr(path.as_ref())?;
        let flags = libc::O_CLOEXEC
            | options.access_mode()?
            | options.creation_mode()?
            | (options.custom_flags & !libc::O_ACCMODE);
        let mode = options.mode;

        Op::submit_with(Open { path, flags, mode })
    }
}

impl OpAble for Open {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::OpenAt::new(types::Fd(libc::AT_FDCWD), self.path.as_c_str().as_ptr())
            .flags(self.flags)
            .mode(self.mode)
            .build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall_u32!(open(
            self.path.as_c_str().as_ptr(),
            self.flags,
            self.mode as libc::c_int
        ))
    }
}
