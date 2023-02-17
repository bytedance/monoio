use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(all(unix, feature = "legacy"))]
use crate::driver::legacy::ready::Direction;

pub(crate) struct PollAdd {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,
    // true: read; false: write
    is_read: bool,
    #[cfg(all(unix, feature = "legacy"))]
    relaxed: bool,
}

impl Op<PollAdd> {
    pub(crate) fn poll_read(fd: &SharedFd, _relaxed: bool) -> io::Result<Op<PollAdd>> {
        Op::submit_with(PollAdd {
            fd: fd.clone(),
            is_read: true,
            #[cfg(all(unix, feature = "legacy"))]
            relaxed: _relaxed,
        })
    }

    pub(crate) fn poll_write(fd: &SharedFd, _relaxed: bool) -> io::Result<Op<PollAdd>> {
        Op::submit_with(PollAdd {
            fd: fd.clone(),
            is_read: false,
            #[cfg(all(unix, feature = "legacy"))]
            relaxed: _relaxed,
        })
    }

    pub(crate) async fn wait(self) -> io::Result<()> {
        let complete = self.await;
        complete.meta.result.map(|_| ())
    }
}

impl OpAble for PollAdd {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::PollAdd::new(
            types::Fd(self.fd.raw_fd()),
            if self.is_read {
                libc::POLLIN as _
            } else {
                libc::POLLOUT as _
            },
        )
        .build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd.registered_index().map(|idx| {
            (
                if self.is_read {
                    Direction::Read
                } else {
                    Direction::Write
                },
                idx,
            )
        })
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        if !self.relaxed {
            use std::{io::ErrorKind, os::fd::AsRawFd};

            let mut pollfd = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: if self.is_read {
                    libc::POLLIN as _
                } else {
                    libc::POLLOUT as _
                },
                revents: 0,
            };
            let ret = crate::syscall_u32!(poll(&mut pollfd as *mut _, 1, 0))?;
            if ret == 0 {
                return Err(ErrorKind::WouldBlock.into());
            }
        }
        Ok(0)
    }
}
