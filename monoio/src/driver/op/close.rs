use super::Op;

use std::io;
use std::os::unix::io::RawFd;

pub(crate) struct Close {
    #[allow(unused)]
    fd: RawFd,
}

impl Op<Close> {
    pub(crate) fn close(fd: RawFd) -> io::Result<Op<Close>> {
        use io_uring::{opcode, types};

        Op::try_submit_with(Close { fd }, |_| opcode::Close::new(types::Fd(fd)).build())
    }
}
