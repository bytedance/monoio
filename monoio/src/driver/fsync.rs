use crate::driver::{Op, SharedFd};

use std::io;

pub(crate) struct Fsync {
    #[allow(unused)]
    fd: SharedFd,
}

impl Op<Fsync> {
    pub(crate) fn fsync(fd: &SharedFd) -> io::Result<Op<Fsync>> {
        use io_uring::{opcode, types};

        Op::submit_with(Fsync { fd: fd.clone() }, |_| {
            opcode::Fsync::new(types::Fd(fd.raw_fd())).build()
        })
    }

    pub(crate) fn datasync(fd: &SharedFd) -> io::Result<Op<Fsync>> {
        use io_uring::{opcode, types};

        Op::submit_with(Fsync { fd: fd.clone() }, |_| {
            opcode::Fsync::new(types::Fd(fd.raw_fd()))
                .flags(types::FsyncFlags::DATASYNC)
                .build()
        })
    }
}
