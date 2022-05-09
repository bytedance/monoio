use super::{Op, OpAble};

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
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::Close::new(types::Fd(self.fd)).build()
    }
}
