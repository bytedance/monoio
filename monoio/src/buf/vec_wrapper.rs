#[cfg(windows)]
use {std::ops::Add, windows_sys::Win32::Networking::WinSock::WSABUF};

use super::{IoBuf, IoBufMut, IoVecBuf, IoVecBufMut};

pub(crate) struct IoVecMeta {
    #[cfg(unix)]
    data: Vec<libc::iovec>,
    #[cfg(windows)]
    data: Vec<WSABUF>,
    offset: usize,
    len: usize,
}

/// Read IoVecBuf meta data into a Vec.
pub(crate) fn read_vec_meta<T: IoVecBuf>(buf: &T) -> IoVecMeta {
    #[cfg(unix)]
    {
        let ptr = buf.read_iovec_ptr();
        let iovec_len = buf.read_iovec_len();

        let mut data = Vec::with_capacity(iovec_len);
        let mut len = 0;
        for i in 0..iovec_len {
            let iovec = unsafe { *ptr.add(i) };
            data.push(iovec);
            len += iovec.iov_len;
        }
        IoVecMeta {
            data,
            offset: 0,
            len,
        }
    }
    #[cfg(windows)]
    {
        let ptr = buf.read_wsabuf_ptr();
        let wsabuf_len = buf.read_wsabuf_len();

        let mut data = Vec::with_capacity(wsabuf_len);
        let mut len = 0;
        for i in 0..wsabuf_len {
            let wsabuf = unsafe { *ptr.add(i) };
            data.push(wsabuf);
            len += wsabuf.len;
        }
        let len = len as _;
        IoVecMeta {
            data,
            offset: 0,
            len,
        }
    }
}

/// Read IoVecBufMut meta data into a Vec.
pub(crate) fn write_vec_meta<T: IoVecBufMut>(buf: &mut T) -> IoVecMeta {
    #[cfg(unix)]
    {
        let ptr = buf.write_iovec_ptr();
        let iovec_len = buf.write_iovec_len();

        let mut data = Vec::with_capacity(iovec_len);
        let mut len = 0;
        for i in 0..iovec_len {
            let iovec = unsafe { *ptr.add(i) };
            data.push(iovec);
            len += iovec.iov_len;
        }
        IoVecMeta {
            data,
            offset: 0,
            len,
        }
    }
    #[cfg(windows)]
    {
        let ptr = buf.write_wsabuf_ptr();
        let wsabuf_len = buf.write_wsabuf_len();

        let mut data = Vec::with_capacity(wsabuf_len);
        let mut len = 0;
        for i in 0..wsabuf_len {
            let wsabuf = unsafe { *ptr.add(i) };
            data.push(wsabuf);
            len += wsabuf.len;
        }
        let len = len as _;
        IoVecMeta {
            data,
            offset: 0,
            len,
        }
    }
}

impl IoVecMeta {
    #[allow(unused_mut)]
    pub(crate) fn consume(&mut self, mut amt: usize) {
        #[cfg(unix)]
        {
            if amt == 0 {
                return;
            }
            let mut offset = self.offset;
            while let Some(iovec) = self.data.get_mut(offset) {
                match iovec.iov_len.cmp(&amt) {
                    std::cmp::Ordering::Less => {
                        amt -= iovec.iov_len;
                        offset += 1;
                        continue;
                    }
                    std::cmp::Ordering::Equal => {
                        offset += 1;
                        self.offset = offset;
                        return;
                    }
                    std::cmp::Ordering::Greater => {
                        let _ = unsafe { iovec.iov_base.add(amt) };
                        iovec.iov_len -= amt;
                        self.offset = offset;
                        return;
                    }
                }
            }
            panic!("try to consume more than owned")
        }
        #[cfg(windows)]
        {
            let mut amt = amt as _;
            if amt == 0 {
                return;
            }
            let mut offset = self.offset;
            while let Some(wsabuf) = self.data.get_mut(offset) {
                match wsabuf.len.cmp(&amt) {
                    std::cmp::Ordering::Less => {
                        amt -= wsabuf.len;
                        offset += 1;
                        continue;
                    }
                    std::cmp::Ordering::Equal => {
                        offset += 1;
                        self.offset = offset;
                        return;
                    }
                    std::cmp::Ordering::Greater => {
                        _ = wsabuf.len.add(amt);
                        wsabuf.len -= amt;
                        self.offset = offset;
                        return;
                    }
                }
            }
            panic!("try to consume more than owned")
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }
}

unsafe impl IoVecBuf for IoVecMeta {
    #[cfg(unix)]
    fn read_iovec_ptr(&self) -> *const libc::iovec {
        unsafe { self.data.as_ptr().add(self.offset) }
    }
    #[cfg(unix)]
    fn read_iovec_len(&self) -> usize {
        self.data.len()
    }
    #[cfg(windows)]
    fn read_wsabuf_ptr(&self) -> *const WSABUF {
        unsafe { self.data.as_ptr().add(self.offset) }
    }
    #[cfg(windows)]
    fn read_wsabuf_len(&self) -> usize {
        self.data.len()
    }
}

unsafe impl IoVecBufMut for IoVecMeta {
    #[cfg(unix)]
    fn write_iovec_ptr(&mut self) -> *mut libc::iovec {
        unsafe { self.data.as_mut_ptr().add(self.offset) }
    }

    #[cfg(unix)]
    fn write_iovec_len(&mut self) -> usize {
        self.data.len()
    }

    #[cfg(windows)]
    fn write_wsabuf_ptr(&mut self) -> *mut WSABUF {
        unsafe { self.data.as_mut_ptr().add(self.offset) }
    }

    #[cfg(windows)]
    fn write_wsabuf_len(&mut self) -> usize {
        self.data.len()
    }

    unsafe fn set_init(&mut self, pos: usize) {
        self.consume(pos)
    }
}

impl<'t, T: IoBuf> From<&'t T> for IoVecMeta {
    fn from(buf: &'t T) -> Self {
        let ptr = buf.read_ptr() as *const _ as *mut _;
        let len = buf.bytes_init() as _;
        #[cfg(unix)]
        let item = libc::iovec {
            iov_base: ptr,
            iov_len: len,
        };
        #[cfg(windows)]
        let item = WSABUF { buf: ptr, len };
        Self {
            data: vec![item],
            offset: 0,
            len: 1,
        }
    }
}

impl<'t, T: IoBufMut> From<&'t mut T> for IoVecMeta {
    fn from(buf: &'t mut T) -> Self {
        let ptr = buf.write_ptr() as *mut _;
        let len = buf.bytes_total() as _;
        #[cfg(unix)]
        let item = libc::iovec {
            iov_base: ptr,
            iov_len: len,
        };
        #[cfg(windows)]
        let item = WSABUF { buf: ptr, len };
        Self {
            data: vec![item],
            offset: 0,
            len: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buf::VecBuf;

    #[test]
    fn test_read_vec_meta() {
        let iovec = VecBuf::from(vec![vec![0; 10], vec![0; 20], vec![0; 30]]);
        let meta = read_vec_meta(&iovec);
        assert_eq!(meta.len(), 60);
        assert_eq!(meta.data.len(), 3);
        #[cfg(unix)]
        {
            assert_eq!(meta.data[0].iov_len, 10);
            assert_eq!(meta.data[1].iov_len, 20);
            assert_eq!(meta.data[2].iov_len, 30);
        }
        #[cfg(windows)]
        {
            assert_eq!(meta.data[0].len, 10);
            assert_eq!(meta.data[1].len, 20);
            assert_eq!(meta.data[2].len, 30);
        }
    }

    #[test]
    fn test_write_vec_meta() {
        let mut iovec = VecBuf::from(vec![vec![0; 10], vec![0; 20], vec![0; 30]]);
        let meta = write_vec_meta(&mut iovec);
        assert_eq!(meta.len(), 60);
        assert_eq!(meta.data.len(), 3);
        #[cfg(unix)]
        {
            assert_eq!(meta.data[0].iov_len, 10);
            assert_eq!(meta.data[1].iov_len, 20);
            assert_eq!(meta.data[2].iov_len, 30);
        }
        #[cfg(windows)]
        {
            assert_eq!(meta.data[0].len, 10);
            assert_eq!(meta.data[1].len, 20);
            assert_eq!(meta.data[2].len, 30);
        }
    }
}
