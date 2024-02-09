use std::{io, net::SocketAddr};

#[cfg(all(target_os = "linux", feature = "iouring"))]
use io_uring::{opcode, types};
#[cfg(unix)]
use {
    crate::net::unix::SocketAddr as UnixSocketAddr,
    libc::{socklen_t, AF_INET, AF_INET6},
    std::mem::{transmute, MaybeUninit},
    std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6},
};
#[cfg(all(windows, any(feature = "legacy", feature = "poll-io")))]
use {
    crate::syscall, std::os::windows::io::AsRawSocket,
    windows_sys::Win32::Networking::WinSock::recv,
};
#[cfg(all(unix, any(feature = "legacy", feature = "poll-io")))]
use {crate::syscall_u32, std::os::unix::prelude::AsRawFd};

use super::{super::shared_fd::SharedFd, Op, OpAble};
#[cfg(any(feature = "legacy", feature = "poll-io"))]
use crate::driver::ready::Direction;
use crate::{buf::IoBufMut, BufResult};

pub(crate) struct Recv<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    /// Reference to the in-flight buffer.
    pub(crate) buf: T,
}

impl<T: IoBufMut> Op<Recv<T>> {
    pub(crate) fn recv(fd: SharedFd, buf: T) -> io::Result<Self> {
        Op::submit_with(Recv { fd, buf })
    }

    #[allow(unused)]
    pub(crate) fn recv_raw(fd: &SharedFd, buf: T) -> Recv<T> {
        Recv {
            fd: fd.clone(),
            buf,
        }
    }

    pub(crate) async fn read(self) -> BufResult<usize, T> {
        let complete = self.await;
        let res = complete.meta.result.map(|v| v as _);
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

impl<T: IoBufMut> OpAble for Recv<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::Recv::new(
            types::Fd(self.fd.raw_fd()),
            self.buf.write_ptr(),
            self.buf.bytes_total() as _,
        )
        .build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd.registered_index().map(|idx| (Direction::Read, idx))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_fd();
        syscall_u32!(recv(
            fd,
            self.buf.write_ptr() as _,
            self.buf.bytes_total().min(u32::MAX as usize),
            0
        ))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_socket();
        syscall!(
            recv(
                fd as _,
                self.buf.write_ptr(),
                self.buf.bytes_total() as _,
                0
            ),
            PartialOrd::ge,
            0
        )
    }
}

pub(crate) struct RecvMsg<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    /// Reference to the in-flight buffer.
    pub(crate) buf: T,
    #[cfg(unix)]
    pub(crate) info: Box<(
        MaybeUninit<libc::sockaddr_storage>,
        [libc::iovec; 1],
        libc::msghdr,
    )>,
}

#[cfg(unix)]
impl<T: IoBufMut> Op<RecvMsg<T>> {
    pub(crate) fn recv_msg(fd: SharedFd, mut buf: T) -> io::Result<Self> {
        let iovec = [libc::iovec {
            iov_base: buf.write_ptr() as *mut _,
            iov_len: buf.bytes_total(),
        }];
        let mut info: Box<(
            MaybeUninit<libc::sockaddr_storage>,
            [libc::iovec; 1],
            libc::msghdr,
        )> = Box::new((MaybeUninit::uninit(), iovec, unsafe { std::mem::zeroed() }));

        info.2.msg_iov = info.1.as_mut_ptr();
        info.2.msg_iovlen = 1;
        info.2.msg_name = &mut info.0 as *mut _ as *mut libc::c_void;
        info.2.msg_namelen = std::mem::size_of::<libc::sockaddr_storage>() as socklen_t;

        Op::submit_with(RecvMsg { fd, buf, info })
    }

    pub(crate) async fn wait(self) -> BufResult<(usize, SocketAddr), T> {
        let complete = self.await;
        let res = complete.meta.result.map(|v| v as _);
        let mut buf = complete.data.buf;

        let res = res.map(|n| {
            let storage = unsafe { complete.data.info.0.assume_init() };

            let addr = unsafe {
                match storage.ss_family as libc::c_int {
                    AF_INET => {
                        // Safety: if the ss_family field is AF_INET then storage must be a
                        // sockaddr_in.
                        let addr: &libc::sockaddr_in = transmute(&storage);
                        let ip = Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes());
                        let port = u16::from_be(addr.sin_port);
                        SocketAddr::V4(SocketAddrV4::new(ip, port))
                    }
                    AF_INET6 => {
                        // Safety: if the ss_family field is AF_INET6 then storage must be a
                        // sockaddr_in6.
                        let addr: &libc::sockaddr_in6 = transmute(&storage);
                        let ip = Ipv6Addr::from(addr.sin6_addr.s6_addr);
                        let port = u16::from_be(addr.sin6_port);
                        SocketAddr::V6(SocketAddrV6::new(
                            ip,
                            port,
                            addr.sin6_flowinfo,
                            addr.sin6_scope_id,
                        ))
                    }
                    _ => {
                        unreachable!()
                    }
                }
            };

            // Safety: the kernel wrote `n` bytes to the buffer.
            unsafe {
                buf.set_init(n);
            }

            (n, addr)
        });
        (res, buf)
    }
}

#[cfg(windows)]
impl<T: IoBufMut> Op<RecvMsg<T>> {
    #[allow(unused_mut, unused_variables)]
    pub(crate) fn recv_msg(fd: SharedFd, mut buf: T) -> io::Result<Self> {
        unimplemented!()
    }

    pub(crate) async fn wait(self) -> BufResult<(usize, SocketAddr), T> {
        unimplemented!()
    }
}

impl<T: IoBufMut> OpAble for RecvMsg<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::RecvMsg::new(types::Fd(self.fd.raw_fd()), &mut self.info.2 as *mut _).build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd.registered_index().map(|idx| (Direction::Read, idx))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_fd();
        syscall_u32!(recvmsg(fd, &mut self.info.2 as *mut _, 0))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let _fd = self.fd.as_raw_socket();
        unimplemented!();
    }
}

pub(crate) struct RecvMsgUnix<T> {
    /// Holds a strong ref to the FD, preventing the file from being closed
    /// while the operation is in-flight.
    #[allow(unused)]
    fd: SharedFd,

    /// Reference to the in-flight buffer.
    pub(crate) buf: T,
    #[cfg(unix)]
    pub(crate) info: Box<(
        MaybeUninit<libc::sockaddr_storage>,
        [libc::iovec; 1],
        libc::msghdr,
    )>,
}

#[cfg(unix)]
impl<T: IoBufMut> Op<RecvMsgUnix<T>> {
    pub(crate) fn recv_msg_unix(fd: SharedFd, mut buf: T) -> io::Result<Self> {
        let iovec = [libc::iovec {
            iov_base: buf.write_ptr() as *mut _,
            iov_len: buf.bytes_total(),
        }];
        let mut info: Box<(
            MaybeUninit<libc::sockaddr_storage>,
            [libc::iovec; 1],
            libc::msghdr,
        )> = Box::new((MaybeUninit::uninit(), iovec, unsafe { std::mem::zeroed() }));

        info.2.msg_iov = info.1.as_mut_ptr();
        info.2.msg_iovlen = 1;
        info.2.msg_name = &mut info.0 as *mut _ as *mut libc::c_void;
        info.2.msg_namelen = std::mem::size_of::<libc::sockaddr_storage>() as socklen_t;

        Op::submit_with(RecvMsgUnix { fd, buf, info })
    }

    pub(crate) async fn wait(self) -> BufResult<(usize, UnixSocketAddr), T> {
        let complete = self.await;
        let res = complete.meta.result.map(|v| v as _);
        let mut buf = complete.data.buf;

        let res = res.map(|n| {
            let storage = unsafe { complete.data.info.0.assume_init() };
            let name_len = complete.data.info.2.msg_namelen;

            let addr = unsafe {
                let addr: &libc::sockaddr_un = transmute(&storage);
                UnixSocketAddr::from_parts(*addr, name_len)
            };

            // Safety: the kernel wrote `n` bytes to the buffer.
            unsafe {
                buf.set_init(n);
            }

            (n, addr)
        });
        (res, buf)
    }
}

#[cfg(windows)]
impl<T: IoBufMut> Op<RecvMsgUnix<T>> {
    #[allow(unused_mut, unused_variables)]
    pub(crate) fn recv_msg_unix(fd: SharedFd, mut buf: T) -> io::Result<Self> {
        unimplemented!()
    }

    pub(crate) async fn wait(self) -> BufResult<(usize, SocketAddr), T> {
        unimplemented!()
    }
}

impl<T: IoBufMut> OpAble for RecvMsgUnix<T> {
    #[cfg(all(target_os = "linux", feature = "iouring"))]
    fn uring_op(&mut self) -> io_uring::squeue::Entry {
        opcode::RecvMsg::new(types::Fd(self.fd.raw_fd()), &mut self.info.2 as *mut _).build()
    }

    #[cfg(any(feature = "legacy", feature = "poll-io"))]
    #[inline]
    fn legacy_interest(&self) -> Option<(Direction, usize)> {
        self.fd.registered_index().map(|idx| (Direction::Read, idx))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), unix))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let fd = self.fd.as_raw_fd();
        syscall_u32!(recvmsg(fd, &mut self.info.2 as *mut _, 0))
    }

    #[cfg(all(any(feature = "legacy", feature = "poll-io"), windows))]
    fn legacy_call(&mut self) -> io::Result<u32> {
        let _fd = self.fd.as_raw_socket();
        unimplemented!();
    }
}
