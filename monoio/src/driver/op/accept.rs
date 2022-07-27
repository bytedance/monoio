use std::{
    io,
    mem::{size_of, MaybeUninit},
};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(all(unix, feature = "legacy"))]
use {
    crate::{driver::legacy::ready::Direction, syscall_u32},
    std::os::unix::prelude::AsRawFd,
};

use super::{super::shared_fd::SharedFd, Op, OpAble};

/// Accept
pub(crate) struct Accept {
    #[allow(unused)]
    pub(crate) fd: SharedFd,
    #[cfg(unix)]
    pub(crate) addr: Box<(MaybeUninit<libc::sockaddr_storage>, libc::socklen_t)>,
}

impl Op<Accept> {
    #[cfg(unix)]
    /// Accept a connection
    pub(crate) fn accept(fd: &SharedFd) -> io::Result<Self> {
        Op::submit_with(Accept {
            fd: fd.clone(),
            addr: Box::new((
                MaybeUninit::uninit(),
                size_of::<libc::sockaddr_storage>() as libc::socklen_t,
            )),
        })
    }
}

impl OpAble for Accept {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::Accept::new(
            types::Fd(self.fd.raw_fd()),
            self.addr.0.as_mut_ptr() as *mut _,
            &mut self.addr.1,
        )
        .build()
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd.registered_index().map(|idx| (Direction::Read, idx))
    }

    #[cfg(all(unix, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_fd();
        let addr = self.addr.0.as_mut_ptr() as *mut _;
        let len = &mut self.addr.1;
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
