use crate::buf::IoBufMut;
use crate::driver::{Op, SharedFd};
use crate::BufResult;

use std::io;

pub(crate) struct Recv<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    /// Reference to the in-flight buffer.
    pub(crate) buf: T,
}

impl<T: IoBufMut> Op<Recv<T>> {
    pub(crate) fn recv(fd: &SharedFd, buf: T) -> io::Result<Self> {
        use io_uring::{opcode, types};

        Op::submit_with(
            Recv {
                fd: fd.clone(),
                buf,
            },
            |recv| {
                opcode::Recv::new(
                    types::Fd(fd.raw_fd()),
                    recv.buf.write_ptr(),
                    recv.buf.bytes_total() as _,
                )
                .build()
            },
        )
    }

    pub(crate) async fn read(self) -> BufResult<usize, T> {
        let complete = self.await;
        let res = complete.result.map(|v| v as _);
        let mut buf = complete.data.buf;

        if let Ok(n) = res {
            // Safety: the kernel wrote `n` bytes to the buffer.
            unsafe {
                buf.set_init(n);
            }
        }
        (res, buf)
    }
}
