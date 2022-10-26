//! Splice related trait and default impl.

use std::future::Future;

use super::as_fd::{AsReadFd, AsWriteFd};
use crate::{driver::op::Op, net::Pipe};

/// Splice data from self to pipe.
pub trait SpliceSource {
    /// The returned future of splice_to_pipe
    type SpliceFuture<'a>: Future<Output = std::io::Result<u32>>
    where
        Self: 'a;
    /// Splice data from self to pipe.
    fn splice_to_pipe<'a>(&'a mut self, pipe: &'a mut Pipe, len: u32) -> Self::SpliceFuture<'_>;
}

/// Splice data from self from pipe.
pub trait SpliceDestination {
    /// The returned future of splice_from_pipe
    type SpliceFuture<'a>: Future<Output = std::io::Result<u32>>
    where
        Self: 'a;
    /// Splice data from self from pipe.
    fn splice_from_pipe<'a>(&'a mut self, pipe: &'a mut Pipe, len: u32) -> Self::SpliceFuture<'_>;
}

impl<T: AsReadFd> SpliceSource for T {
    type SpliceFuture<'a> = impl Future<Output = std::io::Result<u32>> + 'a where Self: 'a;

    #[inline]
    fn splice_to_pipe<'a>(&'a mut self, pipe: &'a mut Pipe, len: u32) -> Self::SpliceFuture<'_> {
        async move {
            Op::splice_to_pipe(self.as_reader_fd().as_ref(), &pipe.fd, len)?
                .splice()
                .await
        }
    }
}

impl<T: AsWriteFd> SpliceDestination for T {
    type SpliceFuture<'a> = impl Future<Output = std::io::Result<u32>> + 'a where Self: 'a;

    #[inline]
    fn splice_from_pipe<'a>(&'a mut self, pipe: &'a mut Pipe, len: u32) -> Self::SpliceFuture<'_> {
        async move {
            Op::splice_from_pipe(&pipe.fd, self.as_writer_fd().as_ref(), len)?
                .splice()
                .await
        }
    }
}
