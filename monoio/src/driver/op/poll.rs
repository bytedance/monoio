use std::io;

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use super::{driver::ready::Direction, MaybeFd};

pub(crate) struct PollAdd {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,
    // true: read; false: write
    is_read: bool,
    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    relaxed: bool,
}

impl Op<PollAdd> {
    pub(crate) fn poll_read(fd: &SharedFd, _relaxed: bool) -> io::Result<Op<PollAdd>> {
        Op::submit_with(PollAdd {
            fd: fd.clone(),
            is_read: true,
            #[cfg(any(feature = "legacy", feature = "poll-io"))]
            relaxed: _relaxed,
        })
    }

    pub(crate) fn poll_write(fd: &SharedFd, _relaxed: bool) -> io::Result<Op<PollAdd>> {
        Op::submit_with(PollAdd {
            fd: fd.clone(),
            is_read: false,
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
        use io_uring::{opcode, types};

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

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
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

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), not(windows)))]
    fn legacy_call(&mut self) -> io::Result<MaybeFd> {
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
            let ret = crate::syscall!(poll@RAW(&mut pollfd as *mut _, 1, 0))?;
            if ret == 0 {
                return Err(ErrorKind::WouldBlock.into());
            }
        }
        Ok(MaybeFd::new_non_fd(1))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<MaybeFd> {
        use std::{
            io::{Error, ErrorKind},
            os::windows::prelude::AsRawSocket,
        };

        use windows_sys::Win32::Networking::WinSock::{
            WSAGetLastError, WSAPoll, POLLIN, POLLOUT, SOCKET_ERROR, WSAPOLLFD,
        };

        if !self.relaxed {
            let mut pollfd = WSAPOLLFD {
                fd: self.fd.as_raw_socket() as _,
                events: if self.is_read {
                    POLLIN as _
                } else {
                    POLLOUT as _
                },
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
        Ok(MaybeFd::new_non_fd(1))
    }
}
