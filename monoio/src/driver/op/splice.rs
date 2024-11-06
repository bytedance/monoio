//! This module works only on linux.

use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(all(unix, feature = "legacy"))]
use {
    crate::driver::{op::MaybeFd, ready::Direction},
    std::os::unix::prelude::AsRawFd,
};

use super::{super::shared_fd::SharedFd, Op, OpAble};

// Currently our Splice does not support setting offset.
pub(crate) struct Splice {
    fd_in: SharedFd,
    fd_out: SharedFd,
    len: u32,
    direction: SpliceDirection,
}
enum SpliceDirection {
    FromPipe,
    ToPipe,
}

impl Op<Splice> {
    pub(crate) fn splice_to_pipe(
        fd_in: &SharedFd,
        fd_out: &SharedFd,
        len: u32,
    ) -> io::Result<Op<Splice>> {
        Op::submit_with(Splice {
            fd_in: fd_in.clone(),
            fd_out: fd_out.clone(),
            len,
            direction: SpliceDirection::ToPipe,
        })
    }

    pub(crate) fn splice_from_pipe(
        fd_in: &SharedFd,
        fd_out: &SharedFd,
        len: u32,
    ) -> io::Result<Op<Splice>> {
        Op::submit_with(Splice {
            fd_in: fd_in.clone(),
            fd_out: fd_out.clone(),
            len,
            direction: SpliceDirection::FromPipe,
        })
    }

    pub(crate) async fn splice(self) -> io::Result<u32> {
        let complete = self.await;
        complete.meta.result.map(|v| v.into_inner())
    }
}

impl OpAble for Splice {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        const FLAG: u32 = libc::SPLICE_F_MOVE;
        opcode::Splice::new(
            types::Fd(self.fd_in.raw_fd()),
            -1,
            types::Fd(self.fd_out.raw_fd()),
            -1,
            self.len,
        )
        .flags(FLAG)
        .build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        match self.direction {
            SpliceDirection::FromPipe => self
                .fd_out
                .registered_index()
                .map(|idx| (Direction::Write, idx)),
            SpliceDirection::ToPipe => self
                .fd_in
                .registered_index()
                .map(|idx| (Direction::Read, idx)),
        }
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<MaybeFd> {
        const FLAG: u32 = libc::SPLICE_F_MOVE | libc::SPLICE_F_NONBLOCK;
        let fd_in = self.fd_in.as_raw_fd();
        let fd_out = self.fd_out.as_raw_fd();
        let off_in = std::ptr::null_mut::<libc::loff_t>();
        let off_out = std::ptr::null_mut::<libc::loff_t>();
        crate::syscall!(splice@NON_FD(
            fd_in,
            off_in,
            fd_out,
            off_out,
            self.len as usize,
            FLAG
        ))
    }
}
