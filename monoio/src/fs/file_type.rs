use std::{fmt::Debug, os::unix::fs::FileTypeExt};

use libc::mode_t;

/// A structure representing a type of file with accessors for each file type.
#[derive(PartialEq, Eq, Clone, Copy, Hash)]
pub struct FileType {
    pub(crate) mode: mode_t,
}

#[cfg(unix)]
impl FileType {
    /// Returns `true` if this file type is a directory.
    pub fn is_dir(&self) -> bool {
        self.is(libc::S_IFDIR)
    }

    /// Returns `true` if this file type is a regular file.
    pub fn is_file(&self) -> bool {
        self.is(libc::S_IFREG)
    }

    /// Returns `true` if this file type is a symbolic link.
    pub fn is_symlink(&self) -> bool {
        self.is(libc::S_IFLNK)
    }

    pub(crate) fn is(&self, mode: mode_t) -> bool {
        self.masked() == mode
    }

    fn masked(&self) -> mode_t {
        self.mode & libc::S_IFMT
    }
}

impl FileTypeExt for FileType {
    fn is_block_device(&self) -> bool {
        self.is(libc::S_IFBLK)
    }

    fn is_char_device(&self) -> bool {
        self.is(libc::S_IFCHR)
    }

    fn is_fifo(&self) -> bool {
        self.is(libc::S_IFIFO)
    }

    fn is_socket(&self) -> bool {
        self.is(libc::S_IFSOCK)
    }
}

impl Debug for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileType")
            .field("is_file", &self.is_file())
            .field("is_dir", &self.is_dir())
            .field("is_symlink", &self.is_symlink())
            .finish_non_exhaustive()
    }
}
