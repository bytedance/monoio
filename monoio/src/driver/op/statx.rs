#[cfg(unix)]
use std::os::fd::AsRawFd;
use std::{ffi::CString, mem::MaybeUninit, path::Path};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(target_os = "linux")]
use libc::statx;

use super::{Op, OpAble};
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use crate::driver::ready::Direction;
use crate::driver::{shared_fd::SharedFd, util::cstr};

#[derive(Debug)]
pub(crate) struct Statx<T> {
    inner: T,
    flags: i32,
    statx_buf: Box<MaybeUninit<statx>>,
}

type FdStatx = Statx<SharedFd>;

impl Op<FdStatx> {
    /// submit a statx operation
    pub(crate) fn statx_using_fd(fd: &SharedFd, flags: i32) -> std::io::Result<Self> {
        Op::submit_with(Statx {
            inner: fd.clone(),
            flags,
            statx_buf: Box::new(MaybeUninit::uninit()),
        })
    }

    pub(crate) async fn statx_result(self) -> std::io::Result<statx> {
        let complete = self.await;
        complete.meta.result?;

        Ok(unsafe { MaybeUninit::assume_init(*complete.data.statx_buf) })
    }
}

impl OpAble for FdStatx {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        let statxbuf = self.statx_buf.as_mut_ptr() as *mut _;

        opcode::Statx::new(types::Fd(self.inner.as_raw_fd()), c"".as_ptr(), statxbuf)
            .flags(libc::AT_EMPTY_PATH | libc::AT_STATX_SYNC_AS_STAT)
            .mask(libc::STATX_ALL)
            .build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    fn legacy_interest(&self) -> Option<(crate::driver::ready::Direction, usize)> {
        self.inner
            .registered_index()
            .map(|idx| (Direction::Read, idx))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), target_os = "linux"))]
    fn legacy_call(&mut self) -> std::io::Result<u32> {
        use std::os::fd::AsRawFd;

        use crate::syscall_u32;

        syscall_u32!(statx(
            self.inner.as_raw_fd(),
            c"".as_ptr(),
            libc::AT_EMPTY_PATH,
            libc::STATX_ALL,
            self.statx_buf.as_mut_ptr() as *mut _
        ))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> std::io::Result<u32> {
        unimplemented!()
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), target_os = "macos"))]
    fn legacy_call(&mut self) -> std::io::Result<u32> {
        unimplemented!()
    }
}

type PathStatx = Statx<CString>;

impl Op<PathStatx> {
    /// submit a statx operation
    pub(crate) fn statx_using_path<P: AsRef<Path>>(path: P, flags: i32) -> std::io::Result<Self> {
        let path = cstr(path.as_ref())?;
        Op::submit_with(Statx {
            inner: path,
            flags,
            statx_buf: Box::new(MaybeUninit::uninit()),
        })
    }

    pub(crate) async fn statx_result(self) -> std::io::Result<statx> {
        let complete = self.await;
        complete.meta.result?;

        Ok(unsafe { MaybeUninit::assume_init(*complete.data.statx_buf) })
    }
}

impl OpAble for PathStatx {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        let statxbuf = self.statx_buf.as_mut_ptr() as *mut _;

        opcode::Statx::new(types::Fd(libc::AT_FDCWD), self.inner.as_ptr(), statxbuf)
            .flags(self.flags)
            .mask(libc::STATX_ALL)
            .build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    fn legacy_interest(&self) -> Option<(crate::driver::ready::Direction, usize)> {
        None
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), target_os = "linux"))]
    fn legacy_call(&mut self) -> std::io::Result<u32> {
        use crate::syscall_u32;

        syscall_u32!(statx(
            libc::AT_FDCWD,
            self.inner.as_ptr(),
            self.flags,
            libc::STATX_ALL,
            self.statx_buf.as_mut_ptr() as *mut _
        ))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> std::io::Result<u32> {
        unimplemented!()
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), target_os = "macos"))]
    fn legacy_call(&mut self) -> std::io::Result<u32> {
        unimplemented!()
    }
}
