use std::{ffi::CString, path::Path};

#[cfg(any(feature = "legacy", feature = "poll-io"))]
use super::MaybeFd;
use super::{Op, OpAble};
use crate::driver::util::cstr;

pub(crate) struct Rename {
    from: CString,
    to: CString,
}

impl Op<Rename> {
    pub(crate) fn rename(from: &Path, to: &Path) -> std::io::Result<Self> {
        let from = cstr(from)?;
        let to = cstr(to)?;

        Op::submit_with(Rename { from, to })
    }
}

impl OpAble for Rename {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        use io_uring::{opcode::RenameAt, types};
        use libc::AT_FDCWD;

        RenameAt::new(
            types::Fd(AT_FDCWD),
            self.from.as_ptr(),
            types::Fd(AT_FDCWD),
            self.to.as_ptr(),
        )
        .build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    fn legacy_interest(&self) -> Option<(crate::driver::ready::Direction, usize)> {
        None
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> std::io::Result<MaybeFd> {
        crate::syscall!(renameat@NON_FD(
            libc::AT_FDCWD,
            self.from.as_ptr(),
            libc::AT_FDCWD,
            self.to.as_ptr()
        ))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> std::io::Result<MaybeFd> {
        use std::io::{Error, ErrorKind};

        use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING};

        use crate::driver::util::to_wide_string;

        let from = to_wide_string(
            self.from
                .to_str()
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?,
        );

        let to = to_wide_string(
            self.to
                .to_str()
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?,
        );

        crate::syscall!(
            MoveFileExW@NON_FD(from.as_ptr(), to.as_ptr(), MOVEFILE_REPLACE_EXISTING),
            PartialEq::eq,
            0
        )
    }
}
