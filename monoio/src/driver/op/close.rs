#[cfg(unix)]
use std::{io, os::unix::io::RawFd};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};

use super::{Op, OpAble};
#[cfg(all(unix, feature = "legacy"))]
use crate::{driver::legacy::ready::Direction, syscall_u32};

pub(crate) struct Close {
    #[cfg(unix)]
    fd: RawFd,
}

impl Op<Close> {
    #[allow(unused)]
    #[cfg(unix)]
    pub(crate) fn close(fd: RawFd) -> io::Result<Op<Close>> {
        Op::try_submit_with(Close { fd })
    }
}

impl OpAble for Close {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::Close::new(types::Fd(self.fd)).build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall_u32!(close(self.fd))
    }
}
