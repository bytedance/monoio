use crate::{driver::legacy::ready::Direction, syscall_u32};

use super::{Op, OpAble};

#[cfg(target_os = "linux")]
use io_uring::{opcode, types};

use std::{io, os::unix::io::RawFd};

pub(crate) struct Close {
    fd: RawFd,
}

impl Op<Close> {
    pub(crate) fn close(fd: RawFd) -> io::Result<Op<Close>> {
        Op::try_submit_with(Close { fd })
    }
}

impl OpAble for Close {
    #[cfg(target_os = "linux")]
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::Close::new(types::Fd(self.fd)).build()
    }

    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    fn legacy_call(self: &mut std::pin::Pin<Box<Self>>) -> io::Result<u32> {
        syscall_u32!(close(self.fd))
    }
}
