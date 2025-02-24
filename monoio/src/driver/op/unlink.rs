use std::{ffi::CString, io, path::Path};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, squeue::Entry, types::Fd};
#[cfg(all(target_os = "linux", feature = "iouring"))]
use libc::{AT_FDCWD, AT_REMOVEDIR};

use super::{MaybeFd, Op, OpAble};
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use crate::driver::ready::Direction;
use crate::driver::util::cstr;

pub(crate) struct Unlink {
    path: CString,
    remove_dir: bool,
}

impl Op<Unlink> {
    pub(crate) fn unlink<P: AsRef<Path>>(path: P) -> io::Result<Op<Unlink>> {
        let path = cstr(path.as_ref())?;
        Op::submit_with(Unlink {
            path,
            remove_dir: false,
        })
    }

    pub(crate) fn rmdir<P: AsRef<Path>>(path: P) -> io::Result<Op<Unlink>> {
        let path = cstr(path.as_ref())?;
        Op::submit_with(Unlink {
            path,
            remove_dir: true,
        })
    }
}

impl OpAble for Unlink {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> Entry {
        opcode::UnlinkAt::new(Fd(AT_FDCWD), self.path.as_c_str().as_ptr())
            .flags(if self.remove_dir { AT_REMOVEDIR } else { 0 })
            .build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(unix, any(feature = "legacy", feature = "poll-io")))]
    fn legacy_call(&mut self) -> io::Result<MaybeFd> {
        if self.remove_dir {
            crate::syscall!(rmdir@NON_FD(self.path.as_c_str().as_ptr()))
        } else {
            crate::syscall!(unlink@NON_FD(self.path.as_c_str().as_ptr()))
        }
    }

    #[cfg(all(windows, any(feature = "legacy", feature = "poll-io")))]
    fn legacy_call(&mut self) -> std::io::Result<MaybeFd> {
        use std::io::{Error, ErrorKind};

        use windows_sys::Win32::Storage::FileSystem::{DeleteFileW, RemoveDirectoryW};

        use crate::driver::util::to_wide_string;

        let path = to_wide_string(
            self.path
                .to_str()
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?,
        );

        if self.remove_dir {
            crate::syscall!(RemoveDirectoryW@NON_FD(path.as_ptr()), PartialEq::eq, 0)
        } else {
            crate::syscall!(DeleteFileW@NON_FD(path.as_ptr()), PartialEq::eq, 0)
        }
    }
}
