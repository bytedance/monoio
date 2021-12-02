use std::{collections::VecDeque, io::Write};

/// Shared buffer.
/// # Safety
/// Must write valid indices and iovec.
pub unsafe trait Shared: Unpin + 'static {
    /// Write data into dst and return the indices of the written data.
    fn write_all(
        &mut self,
        input: &[u8],
        iovecs: &mut Vec<libc::iovec>,
        indices: &mut Vec<(usize, usize)>,
    ) -> Result<(), std::io::Error>;
}

const BUFFER_BLOCK_BITS: usize = 12;
const BUFFER_BLOCK_SIZE: usize = 1 << BUFFER_BLOCK_BITS; // 4K
const BUFFER_BLOCK_INDEX_MASK: usize = BUFFER_BLOCK_SIZE - 1;

/// Ideally we want the SharedBuf maintain the used and unused part
/// of the whole buffer. Once the part is deleted, the position may
/// be used by the following writings.
/// To avoid copying data when expanding, we may use the linked list
/// as storage instead of Vec.
/// For simple, we implement it as discarding the "hole", and using Vec
/// which may lead to copying.
/// TODO(ihciah): improve it.
#[derive(Default, Clone)]
pub struct SharedBuf {
    bufs: VecDeque<Box<[u8; BUFFER_BLOCK_SIZE]>>,
    len: usize,
}

impl SharedBuf {
    /// New with capacity.
    pub fn with_capacity(cap: usize) -> Self {
        let buffers_len = (cap >> BUFFER_BLOCK_BITS) + 1;
        Self {
            bufs: VecDeque::with_capacity(buffers_len),
            len: 0,
        }
    }
}

unsafe impl Shared for SharedBuf {
    fn write_all(
        &mut self,
        data: &[u8],
        iovecs: &mut Vec<libc::iovec>,
        indices: &mut Vec<(usize, usize)>,
    ) -> Result<(), std::io::Error> {
        let mut block_id = self.len >> BUFFER_BLOCK_BITS;
        let mut block_index = self.len & BUFFER_BLOCK_INDEX_MASK;

        // alloc memory block to make sure the index is valid.
        let tail_block_id = (self.len + data.len()) >> BUFFER_BLOCK_BITS;
        for _ in block_id..tail_block_id {
            self.bufs.push_back(Box::new([0; BUFFER_BLOCK_SIZE]));
        }

        let mut written = 0;
        while written < data.len() {
            let mut w = &mut self.bufs[block_id][block_index..];
            let l = w.write(&data[written..]).unwrap();
            debug_assert_eq!(l, w.len());

            written += l;
            iovecs.push(libc::iovec {
                iov_base: unsafe {
                    self.bufs[block_id].as_mut_ptr().add(block_index) as *mut libc::c_void
                },
                iov_len: l,
            });
            indices.push(((block_id << BUFFER_BLOCK_BITS) | block_index, l));

            block_id += 1;
            block_index = 0;
        }
        self.len += written;
        Ok(())
    }
}
