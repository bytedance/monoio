use std::{ffi::CString, io, path::Path};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, squeue::Entry, types::Fd};
#[cfg(all(target_os = "linux", feature = "iouring"))]
use libc::{AT_FDCWD, AT_REMOVEDIR};

use super::{Op, OpAble};
use crate::driver::util::cstr;
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use crate::driver::{op::MaybeFd, ready::Direction};

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

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    fn legacy_call(&mut self) -> io::Result<MaybeFd> {
        if self.remove_dir {
            crate::syscall!(rmdir@NON_FD(self.path.as_c_str().as_ptr()))
        } else {
            crate::syscall!(unlink@NON_FD(self.path.as_c_str().as_ptr()))
        }
    }
}
