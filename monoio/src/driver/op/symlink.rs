use std::{ffi::CString, io, path::Path};

#[cfg(any(feature = "legacy", feature = "poll-io"))]
use super::{driver::ready::Direction, MaybeFd};
use super::{Op, OpAble};
use crate::driver::util::cstr;

pub(crate) struct Symlink {
    pub(crate) from: CString,
    pub(crate) to: CString,
}
impl Op<Symlink> {
    pub(crate) fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(
        from: P,
        to: Q,
    ) -> io::Result<Op<Symlink>> {
        let from = cstr(from.as_ref())?;
        let to = cstr(to.as_ref())?;
        Op::submit_with(Symlink { from, to })
    }
}

impl OpAble for Symlink {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        use io_uring::{opcode, types};
        let from_ref = self.from.as_c_str().as_ptr();
        let to_ref = self.to.as_c_str().as_ptr();
        opcode::SymlinkAt::new(types::Fd(libc::AT_FDCWD), from_ref, to_ref).build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    fn legacy_call(&mut self) -> std::io::Result<MaybeFd> {
        crate::syscall!(symlink@NON_FD(
            self.from.as_c_str().as_ptr(),
            self.to.as_c_str().as_ptr()
        ))
    }
}
