//! IO utils

mod buf_reader;
mod buf_writer;
mod copy;
mod prefixed_io;

pub use buf_reader::BufReader;
pub use buf_writer::BufWriter;
pub use copy::copy;
#[cfg(all(target_os = "linux", feature = "splice"))]
pub use copy::zero_copy;
pub use prefixed_io::PrefixedReadIo;
