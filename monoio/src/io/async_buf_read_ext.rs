use std::{
    future::Future,
    io::{Error, ErrorKind, Result},
    str::from_utf8,
};

use memchr::memchr;

use crate::io::AsyncBufRead;

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl Drop for Guard<'_> {
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
    /// This function will read bytes from the underlying stream until the delimiter or EOF is
    /// found. Once found, all bytes up to, and including, the delimiter (if found) will be appended
    /// to buf.
    ///
    /// If successful, this function will return the total number of bytes read.
    ///
    /// # Errors
    /// This function will ignore all instances of ErrorKind::Interrupted and will otherwise return
    /// any errors returned by fill_buf.
    fn read_until<'a>(
        &'a mut self,
        byte: u8,
        buf: &'a mut Vec<u8>,
    ) -> impl Future<Output = Result<usize>>;

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
    fn read_line<'a>(&'a mut self, buf: &'a mut String) -> impl Future<Output = Result<usize>>;
}

impl<A> AsyncBufReadExt for A
where
    A: AsyncBufRead + ?Sized,
{
    fn read_until<'a>(
        &'a mut self,
        byte: u8,
        buf: &'a mut Vec<u8>,
    ) -> impl Future<Output = Result<usize>> {
        read_until(self, byte, buf)
    }

    async fn read_line<'a>(&'a mut self, buf: &'a mut String) -> Result<usize> {
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
