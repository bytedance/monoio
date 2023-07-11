use std::{ffi::CString, io, path::Path};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(windows)]
use windows_sys::Win32::{Foundation::INVALID_HANDLE_VALUE, Storage::FileSystem::CreateFileW};

use super::{Op, OpAble};
#[cfg(feature = "legacy")]
use crate::driver::legacy::ready::Direction;
#[cfg(windows)]
use crate::syscall;
#[cfg(all(unix, feature = "legacy"))]
use crate::syscall_u32;
use crate::{driver::util::cstr, fs::OpenOptions};

/// Open a file
pub(crate) struct Open {
    pub(crate) path: CString,
    #[cfg(unix)]
    flags: i32,
    #[cfg(unix)]
    mode: libc::mode_t,
    #[cfg(windows)]
    opts: OpenOptions,
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

    #[cfg(windows)]
    /// Submit a request to open a file.
    pub(crate) fn open<P: AsRef<Path>>(path: P, options: &OpenOptions) -> io::Result<Op<Open>> {
        // Here the path will be copied, so its safe.
        let path = cstr(path.as_ref())?;

        Op::submit_with(Open {
            path,
            opts: options.clone(),
        })
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

    #[cfg(feature = "legacy")]
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

    #[cfg(windows)]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall!(
            CreateFileW(
                self.path.as_c_str().as_ptr(),
                self.opts.access_mode()?,
                self.opts.share_mode,
                self.opts.security_attributes,
                self.opts.creation_mode()?,
                self.opts.get_flags_and_attributes(),
                std::ptr::null_mut(),
            ),
            PartialEq::eq,
            INVALID_HANDLE_VALUE
        )
    }
}
