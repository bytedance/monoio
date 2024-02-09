use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(windows)]
use {
    crate::syscall, std::os::windows::prelude::AsRawHandle,
    windows_sys::Win32::Storage::FileSystem::FlushFileBuffers,
};

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use crate::driver::ready::Direction;
#[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
use crate::syscall_u32;

pub(crate) struct Fsync {
    #[allow(unused)]
    fd: SharedFd,
    #[cfg(target_os = "linux")]
    data_sync: bool,
}

impl Op<Fsync> {
    pub(crate) fn fsync(fd: &SharedFd) -> io::Result<Op<Fsync>> {
        Op::submit_with(Fsync {
            fd: fd.clone(),
            #[cfg(target_os = "linux")]
            data_sync: false,
        })
    }

    pub(crate) fn datasync(fd: &SharedFd) -> io::Result<Op<Fsync>> {
        Op::submit_with(Fsync {
            fd: fd.clone(),
            #[cfg(target_os = "linux")]
            data_sync: true,
        })
    }
}

impl OpAble for Fsync {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        let mut opc = opcode::Fsync::new(types::Fd(self.fd.raw_fd()));
        if self.data_sync {
            opc = opc.flags(types::FsyncFlags::DATASYNC)
        }
        opc.build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall!(
            FlushFileBuffers(self.fd.as_raw_handle() as _),
            PartialEq::eq,
            0
        )
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        #[cfg(target_os = "linux")]
        if self.data_sync {
            syscall_u32!(fdatasync(self.fd.raw_fd()))
        } else {
            syscall_u32!(fsync(self.fd.raw_fd()))
        }
        #[cfg(not(target_os = "linux"))]
        syscall_u32!(fsync(self.fd.raw_fd()))
    }
}
