use super::{super::shared_fd::SharedFd, Op, OpAble};
use crate::{driver::legacy::ready::Direction, syscall_u32};

#[cfg(target_os = "linux")]
use io_uring::{opcode, types};

use std::{
    io,
    mem::{size_of, MaybeUninit},
    os::unix::prelude::AsRawFd,
};

/// Accept
pub(crate) struct Accept {
    #[allow(unused)]
    pub(crate) fd: SharedFd,
    pub(crate) addr: MaybeUninit<libc::sockaddr_storage>,
    pub(crate) addrlen: libc::socklen_t,
}

impl Op<Accept> {
    /// Accept a connection
    pub(crate) fn accept(fd: &SharedFd) -> io::Result<Self> {
        Op::submit_with(Accept {
            fd: fd.clone(),
            addr: MaybeUninit::uninit(),
            addrlen: size_of::<libc::sockaddr_storage>() as libc::socklen_t,
        })
    }
}

impl OpAble for Accept {
    #[cfg(target_os = "linux")]
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::Accept::new(
            types::Fd(self.fd.raw_fd()),
            self.addr.as_mut_ptr() as *mut _,
            &mut self.addrlen,
        )
        .build()
    }

    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd.registered_index().map(|idx| (Direction::Read, idx))
    }

    fn legacy_call(self: &mut std::pin::Pin<Box<Self>>) -> io::Result<u32> {
        let fd = self.fd.as_raw_fd();
        let addr = self.addr.as_mut_ptr() as *mut _;
        let len = &mut self.addrlen;
        // Here I use copied some code from mio because I don't want the convertion.

        // On platforms that support it we can use `accept4(2)` to set `NONBLOCK`
        // and `CLOEXEC` in the call to accept the connection.
        #[cfg(any(
            // Android x86's seccomp profile forbids calls to `accept4(2)`
            // See https://github.com/tokio-rs/mio/issues/1445 for details
            all(
                not(target_arch="x86"),
                target_os = "android"
            ),
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "illumos",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        return syscall_u32!(accept4(
            fd,
            addr,
            len,
            libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
        ));

        // But not all platforms have the `accept4(2)` call. Luckily BSD (derived)
        // OSes inherit the non-blocking flag from the listener, so we just have to
        // set `CLOEXEC`.
        #[cfg(any(
            all(target_arch = "x86", target_os = "android"),
            target_os = "ios",
            target_os = "macos",
            target_os = "redox"
        ))]
        return {
            let stream_fd = syscall_u32!(accept(fd, addr, len))? as i32;
            syscall_u32!(fcntl(stream_fd, libc::F_SETFD, libc::FD_CLOEXEC))
                .and_then(|_| syscall_u32!(fcntl(stream_fd, libc::F_SETFL, libc::O_NONBLOCK)))
                .map_err(|e| {
                    let _ = syscall_u32!(close(stream_fd));
                    e
                })?;
            Ok(stream_fd as _)
        };
    }
}
