use std::{ffi::CString, path::Path};

#[cfg(unix)]
use libc::mode_t;

#[cfg(any(feature = "legacy", feature = "poll-io"))]
use super::MaybeFd;
use super::{Op, OpAble};
use crate::driver::util::cstr;

pub(crate) struct MkDir {
    path: CString,
    #[cfg(unix)]
    mode: mode_t,
}

impl Op<MkDir> {
    #[cfg(unix)]
    pub(crate) fn mkdir<P: AsRef<Path>>(path: P, mode: mode_t) -> std::io::Result<Op<MkDir>> {
        let path = cstr(path.as_ref())?;
        Op::submit_with(MkDir { path, mode })
    }

    #[cfg(windows)]
    pub(crate) fn mkdir<P: AsRef<Path>>(path: P) -> std::io::Result<Op<MkDir>> {
        let path = cstr(path.as_ref())?;
        Op::submit_with(MkDir { path })
    }
}

impl OpAble for MkDir {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        use io_uring::{opcode, types};

        opcode::MkDirAt::new(types::Fd(libc::AT_FDCWD), self.path.as_ptr())
            .mode(self.mode)
            .build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(crate::driver::ready::Direction, usize)> {
        None
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> std::io::Result<MaybeFd> {
        crate::syscall!(mkdirat@NON_FD(
            libc::AT_FDCWD,
            self.path.as_ptr(),
            self.mode
        ))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> std::io::Result<MaybeFd> {
        use std::io::{Error, ErrorKind};

        use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;

        use crate::driver::util::to_wide_string;

        let path = to_wide_string(
            self.path
                .to_str()
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?,
        );

        // Currently, we don't check the validness of the path.
        // Should we port the Rust std lib to check the validness?
        crate::syscall!(
            CreateDirectoryW@NON_FD(path.as_ptr(), std::ptr::null()),
            PartialEq::eq,
            0
        )
    }
}
