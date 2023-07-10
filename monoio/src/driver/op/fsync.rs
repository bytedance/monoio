use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::FlushFileBuffers;

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(feature = "legacy")]
use crate::driver::legacy::ready::Direction;
#[cfg(windows)]
use crate::syscall;
#[cfg(all(unix, feature = "legacy"))]
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

    #[cfg(feature = "legacy")]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        None
    }

    #[cfg(all(windows, feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall!(
            FlushFileBuffers(self.handle.as_raw_handle()),
            PartialEq::eq,
            0
        )
    }

    #[cfg(all(unix, not(target_os = "linux"), feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        syscall_u32!(fsync(self.fd.raw_fd()))
    }

    #[cfg(all(target_os = "linux", feature = "legacy"))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        if self.data_sync {
            syscall_u32!(fdatasync(self.fd.raw_fd()))
        } else {
            syscall_u32!(fsync(self.fd.raw_fd()))
        }
    }
}
