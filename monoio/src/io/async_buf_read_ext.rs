use core::slice::memchr::memchr;
use std::{
    future::Future,
    io::{Error, ErrorKind, Result},
    ops::Drop,
    str::from_utf8,
};

use crate::io::AsyncBufRead;

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl<'a> Drop for Guard<'a> {
    fn drop(&mut self) {
        unsafe {
            self.buf.set_len(self.len);
        }
    }
}

async fn read_until<A>(r: &mut A, delim: u8, buf: &mut Vec<u8>) -> Result<usize>
where
    A: AsyncBufRead + ?Sized,
{
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf().await {
                Ok(n) => n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };

            match memchr(delim, available) {
                Some(i) => {
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

/// AsyncBufReadExt
pub trait AsyncBufReadExt {
    /// The future of read until Result<usize>
    type ReadUntilFuture<'a>: Future<Output = Result<usize>>
    where
        Self: 'a;

    /// This function will read bytes from the underlying stream until the delimiter or EOF is
    /// found. Once found, all bytes up to, and including, the delimiter (if found) will be appended
    /// to buf.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// # Errors
    /// This function will ignore all instances of ErrorKind::Interrupted and will otherwise return
    /// any errors returned by fill_buf.
    fn read_until<'a>(&'a mut self, byte: u8, buf: &'a mut Vec<u8>) -> Self::ReadUntilFuture<'a>;

    /// The future of read line Result<usize>
    type ReadLineFuture<'a>: Future<Output = Result<usize>>
    where
        Self: 'a;

    /// This function will read bytes from the underlying stream until the newline delimiter (the
    /// 0xA byte) or EOF is found. Once found, all bytes up to, and including, the delimiter (if
    /// found) will be appended to buf.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// If this function returns Ok(0), the stream has reached EOF.
    ///
    /// # Errors
    /// This function has the same error semantics as read_until and will also return an error if
    /// the read bytes are not valid UTF-8. If an I/O error is encountered then buf may contain some
    /// bytes already read in the event that all data read so far was valid UTF-8.
    fn read_line<'a>(&'a mut self, buf: &'a mut String) -> Self::ReadLineFuture<'a>;
}

impl<A> AsyncBufReadExt for A
where
    A: AsyncBufRead + ?Sized,
{
    type ReadUntilFuture<'a> = impl Future<Output = Result<usize>> + 'a where Self: 'a;

    fn read_until<'a>(&'a mut self, byte: u8, buf: &'a mut Vec<u8>) -> Self::ReadUntilFuture<'a> {
        read_until(self, byte, buf)
    }

    type ReadLineFuture<'a> = impl Future<Output = Result<usize>> + 'a where Self: 'a;

    fn read_line<'a>(&'a mut self, buf: &'a mut String) -> Self::ReadLineFuture<'a> {
        async {
            unsafe {
                let mut g = Guard {
                    len: buf.len(),
                    buf: buf.as_mut_vec(),
                };

                let ret = read_until(self, b'\n', g.buf).await;
                if from_utf8(&g.buf[g.len..]).is_err() {
                    ret.and_then(|_| {
                        Err(Error::new(
                            ErrorKind::InvalidData,
                            "stream did not contain valid UTF-8",
                        ))
                    })
                } else {
                    g.len = g.buf.len();
                    ret
                }
            }
        }
    }
}
