use std::{ffi::CString, io, path::Path};

use super::{Op, OpAble};
use crate::driver::util::cstr;

pub(crate) struct Symlink {
    pub(crate) _from: CString,
    pub(crate) _to: CString,
}
impl Op<Symlink> {
    pub(crate) fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(
        from: P,
        to: Q,
    ) -> io::Result<Op<Symlink>> {
        let _from = cstr(from.as_ref())?;
        let _to = cstr(to.as_ref())?;
        Op::submit_with(Symlink { _from, _to })
    }
}

impl OpAble for Symlink {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        use io_uring::{opcode, types};
        let from_ref = self._from.as_c_str().as_ptr();
        let to_ref = self._to.as_c_str().as_ptr();
        opcode::SymlinkAt::new(types::Fd(libc::AT_FDCWD), from_ref, to_ref).build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    fn legacy_interest(&self) -> Option<(crate::driver::ready::Direction, usize)> {
        None
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    fn legacy_call(&mut self) -> std::io::Result<u32> {
        use crate::syscall_u32;
        syscall_u32!(symlink(
            self._from.as_c_str().as_ptr(),
            self._to.as_c_str().as_ptr()
        ))
    }
}
