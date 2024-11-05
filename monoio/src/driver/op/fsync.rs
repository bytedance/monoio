use std::io;

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use super::{driver::ready::Direction, MaybeFd};

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
    fn legacy_call(&mut self) -> io::Result<MaybeFd> {
        use std::os::windows::prelude::AsRawHandle;

        use windows_sys::Win32::Storage::FileSystem::FlushFileBuffers;

        crate::syscall!(
            FlushFileBuffers@NON_FD(self.fd.as_raw_handle() as _),
            PartialEq::eq,
            0
        )
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<MaybeFd> {
        #[cfg(target_os = "linux")]
        if self.data_sync {
            crate::syscall!(fdatasync@NON_FD(self.fd.raw_fd()))
        } else {
            crate::syscall!(fsync@NON_FD(self.fd.raw_fd()))
        }
        #[cfg(not(target_os = "linux"))]
        crate::syscall!(fsync@NON_FD(self.fd.raw_fd()))
    }
}
