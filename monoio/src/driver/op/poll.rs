use std::io;
#[cfg(windows)]
use std::{
    io::{Error, ErrorKind},
    os::windows::prelude::AsRawSocket,
};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(windows)]
use windows_sys::Win32::Networking::WinSock::{
    WSAGetLastError, WSAPoll, POLLIN, POLLOUT, SOCKET_ERROR, WSAPOLLFD,
};

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use crate::driver::ready::Direction;

/// Interest for PollAdd and AsyncFd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PollAddInterest {
    Read,
    Write,
    ReadOrWrite,
}

impl PollAddInterest {
    #[cfg(unix)]
    pub(crate) fn to_flags(self) -> i16 {
        match self {
            PollAddInterest::Read => libc::POLLIN,
            PollAddInterest::Write => libc::POLLOUT,
            PollAddInterest::ReadOrWrite => libc::POLLIN | libc::POLLOUT,
        }
    }

    #[cfg(windows)]
    pub(crate) fn to_flags(self) -> i16 {
        match self {
            PollAddInterest::Read => POLLIN,
            PollAddInterest::Write => POLLOUT,
            PollAddInterest::ReadOrWrite => POLLIN | POLLOUT,
        }
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    pub(crate) fn to_direction(self) -> Direction {
        match self {
            PollAddInterest::Read => Direction::Read,
            PollAddInterest::Write => Direction::Write,
            PollAddInterest::ReadOrWrite => Direction::ReadOrWrite,
        }
    }
}

pub(crate) struct PollAdd {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,
    interest: PollAddInterest,
    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    relaxed: bool,
}

impl Op<PollAdd> {
    pub(crate) fn poll_read(fd: &SharedFd, _relaxed: bool) -> io::Result<Op<PollAdd>> {
        Op::submit_with(PollAdd {
            fd: fd.clone(),
            interest: PollAddInterest::Read,
            #[cfg(any(feature = "legacy", feature = "poll-io"))]
            relaxed: _relaxed,
        })
    }

    pub(crate) fn poll_write(fd: &SharedFd, _relaxed: bool) -> io::Result<Op<PollAdd>> {
        Op::submit_with(PollAdd {
            fd: fd.clone(),
            interest: PollAddInterest::Write,
            #[cfg(any(feature = "legacy", feature = "poll-io"))]
            relaxed: _relaxed,
        })
    }

    pub(crate) fn poll_with_interest(
        fd: &SharedFd,
        interest: PollAddInterest,
        _relaxed: bool,
    ) -> io::Result<Op<PollAdd>> {
        Op::submit_with(PollAdd {
            fd: fd.clone(),
            interest,
            #[cfg(any(feature = "legacy", feature = "poll-io"))]
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
        opcode::PollAdd::new(types::Fd(self.fd.raw_fd()), self.interest.to_flags() as _).build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd
            .registered_index()
            .map(|idx| (self.interest.to_direction(), idx))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        if !self.relaxed {
            use std::{io::ErrorKind, os::fd::AsRawFd};

            let mut pollfd = libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: self.interest.to_flags(),
                revents: 0,
            };
            let ret = unsafe { crate::syscall_u32!(poll(&mut pollfd as *mut _, 1, 0))? };
            if ret == 0 {
                return Err(ErrorKind::WouldBlock.into());
            }
        }
        Ok(0)
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        if !self.relaxed {
            let mut pollfd = WSAPOLLFD {
                fd: self.fd.as_raw_socket() as _,
                events: self.interest.to_flags(),
                revents: 0,
            };
            let ret = unsafe { WSAPoll(&mut pollfd as *mut _, 1, 0) };
            match ret {
                0 => return Err(ErrorKind::WouldBlock.into()),
                SOCKET_ERROR => {
                    let error = unsafe { WSAGetLastError() };
                    return Err(Error::from_raw_os_error(error));
                }
                _ => (),
            }
        }
        Ok(0)
    }
}
