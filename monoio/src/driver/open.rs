use crate::driver::{self, Op};
use crate::fs::OpenOptions;

use std::ffi::CString;
use std::io;
use std::path::Path;

/// Open a file
pub(crate) struct Open {
    pub(crate) path: CString,
}

impl Op<Open> {
    /// Submit a request to open a file.
    pub(crate) fn open<P: AsRef<Path>>(path: P, options: &OpenOptions) -> io::Result<Op<Open>> {
        use io_uring::{opcode, types};

        // Here the path will be copied, so its safe.
        let path = driver::util::cstr(path.as_ref())?;
        let flags = libc::O_CLOEXEC | options.access_mode()? | options.creation_mode()?;

        Op::submit_with(Open { path }, |open| {
            opcode::OpenAt::new(types::Fd(libc::AT_FDCWD), open.path.as_c_str().as_ptr())
                .flags(flags)
                .mode(options.mode)
                .build()
        })
    }
}
