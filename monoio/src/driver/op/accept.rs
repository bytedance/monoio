use super::{super::shared_fd::SharedFd, Op, OpAble};

use io_uring::{opcode, types};
use std::{
    io,
    mem::{size_of, MaybeUninit},
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
    fn uring_op(self: &mut std::pin::Pin<Box<Self>>) -> io_uring::squeue::Entry {
        opcode::Accept::new(
            types::Fd(self.fd.raw_fd()),
            self.addr.as_mut_ptr() as *mut _,
            &mut self.addrlen,
        )
        .build()
    }
}
