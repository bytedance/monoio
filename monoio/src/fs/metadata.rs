#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::{path::Path, time::SystemTime};

#[cfg(unix)]
use libc::mode_t;
use libc::{stat64, statx};

use super::{
    file_type::FileType,
    permissions::{FilePermissions, Permissions},
};
use crate::driver::op::Op;

/// File attributes, not platform-specific.
/// Current implementation is only for unix.
#[cfg(unix)]
pub(crate) struct FileAttr {
    stat: stat64,
    #[cfg(target_os = "linux")]
    statx_extra_fields: Option<StatxExtraFields>,
}

#[cfg(unix)]
impl FileAttr {
    fn size(&self) -> u64 {
        self.stat.st_size as u64
    }

    fn perm(&self) -> FilePermissions {
        FilePermissions {
            mode: (self.stat.st_mode as mode_t),
        }
    }

    fn file_type(&self) -> FileType {
        FileType {
            mode: self.stat.st_mode as mode_t,
        }
    }
}

/// Extra fields that are available in `statx` struct.
#[cfg(target_os = "linux")]
pub(crate) struct StatxExtraFields {
    stx_mask: u32,
    stx_btime: libc::statx_timestamp,
}

/// Convert a `statx` struct to not platform-specific `FileAttr`.
/// Current implementation is only for Linux.
#[cfg(target_os = "linux")]
impl From<statx> for FileAttr {
    fn from(buf: statx) -> Self {
        let mut stat: stat64 = unsafe { std::mem::zeroed() };

        stat.st_dev = libc::makedev(buf.stx_dev_major, buf.stx_dev_minor) as _;
        stat.st_ino = buf.stx_ino as libc::ino64_t;
        stat.st_nlink = buf.stx_nlink as libc::nlink_t;
        stat.st_mode = buf.stx_mode as libc::mode_t;
        stat.st_uid = buf.stx_uid as libc::uid_t;
        stat.st_gid = buf.stx_gid as libc::gid_t;
        stat.st_rdev = libc::makedev(buf.stx_rdev_major, buf.stx_rdev_minor) as _;
        stat.st_size = buf.stx_size as libc::off64_t;
        stat.st_blksize = buf.stx_blksize as libc::blksize_t;
        stat.st_blocks = buf.stx_blocks as libc::blkcnt64_t;
        stat.st_atime = buf.stx_atime.tv_sec as libc::time_t;
        // `i64` on gnu-x86_64-x32, `c_ulong` otherwise.
        stat.st_atime_nsec = buf.stx_atime.tv_nsec as _;
        stat.st_mtime = buf.stx_mtime.tv_sec as libc::time_t;
        stat.st_mtime_nsec = buf.stx_mtime.tv_nsec as _;
        stat.st_ctime = buf.stx_ctime.tv_sec as libc::time_t;
        stat.st_ctime_nsec = buf.stx_ctime.tv_nsec as _;

        let extra = StatxExtraFields {
            stx_mask: buf.stx_mask,
            stx_btime: buf.stx_btime,
        };

        Self {
            stat,
            statx_extra_fields: Some(extra),
        }
    }
}

/// Metadata information about a file.
///
/// This structure is returned from the [`metadata`] or
/// [`symlink_metadata`] function or method and represents known
/// metadata about a file such as its permissions, size, modification
/// times, etc.
#[cfg(unix)]
pub struct Metadata(pub(crate) FileAttr);

impl Metadata {
    /// Returns `true` if this metadata is for a directory.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs;
    ///
    /// #[monoio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("path/to/dir").await?;
    ///
    ///     println!("{:?}", metadata.is_dir());
    ///     Ok(())
    /// }
    /// ```
    pub fn is_dir(&self) -> bool {
        self.0.stat.st_mode & libc::S_IFMT == libc::S_IFDIR
    }

    /// Returns `true` if this metadata is for a regular file.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs;
    ///
    /// #[monoio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt").await?;
    ///
    ///     println!("{:?}", metadata.is_file());
    ///     Ok(())
    /// }
    /// ```
    pub fn is_file(&self) -> bool {
        self.0.stat.st_mode & libc::S_IFMT == libc::S_IFREG
    }

    /// Returns `true` if this metadata is for a symbolic link.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs;
    ///
    /// #[monoio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt").await?;
    ///
    ///     println!("{:?}", metadata.is_symlink());
    ///     Ok(())
    /// }
    /// ```
    pub fn is_symlink(&self) -> bool {
        self.0.stat.st_mode & libc::S_IFMT == libc::S_IFLNK
    }

    /// Returns the size of the file, in bytes, this metadata is for.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs;
    ///
    /// #[monoio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt").await?;
    ///
    ///     println!("{:?}", metadata.len());
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        self.0.size()
    }

    /// Returns the last modification time listed in this metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs;
    ///
    /// #[monoio::main]
    /// async fn main() -> std::io::Result<()> {
    ///    let metadata = fs::metadata("foo.txt").await?;
    ///
    ///    println!("{:?}", metadata.modified());
    ///    Ok(())
    /// }
    pub fn modified(&self) -> std::io::Result<SystemTime> {
        let mtime = self.0.stat.st_mtime;
        let mtime_nsec = self.0.stat.st_mtime_nsec as u32;

        Ok(SystemTime::UNIX_EPOCH + std::time::Duration::new(mtime as u64, mtime_nsec))
    }

    /// Returns the last access time listed in this metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs;
    ///
    /// #[monoio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt").await?;
    ///
    ///     println!("{:?}", metadata.accessed());
    ///     Ok(())
    /// }
    /// ```
    pub fn accessed(&self) -> std::io::Result<SystemTime> {
        let atime = self.0.stat.st_atime;
        let atime_nsec = self.0.stat.st_atime_nsec as u32;

        Ok(SystemTime::UNIX_EPOCH + std::time::Duration::new(atime as u64, atime_nsec))
    }

    /// Returns the creation time listed in this metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs;
    ///
    /// #[monoio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt").await?;
    ///
    ///     println!("{:?}", metadata.created());
    ///     Ok(())
    /// }
    /// ```
    #[cfg(target_os = "linux")]
    pub fn created(&self) -> std::io::Result<SystemTime> {
        if let Some(extra) = self.0.statx_extra_fields.as_ref() {
            return if extra.stx_mask & libc::STATX_BTIME != 0 {
                let btime = extra.stx_btime.tv_sec;
                let btime_nsec = extra.stx_btime.tv_nsec;

                Ok(SystemTime::UNIX_EPOCH + std::time::Duration::new(btime as u64, btime_nsec))
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Creation time is not available",
                ))
            };
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Creation time is not available",
        ))
    }

    /// Returns the permissions of the file this metadata is for.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs;
    ///
    /// #[monoio::main]
    /// async fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt").await?;
    ///
    ///     println!("{:?}", metadata.permissions());
    ///     Ok(())
    /// }
    /// ```
    #[cfg(unix)]
    pub fn permissions(&self) -> Permissions {
        Permissions(self.0.perm())
    }

    /// Returns the file type for this metadata.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::fs;
    ///
    /// #[monoio::main]
    /// fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt").await?;
    ///
    ///     println!("{:?}", metadata.file_type());
    ///     Ok(())
    /// }
    /// ```
    #[cfg(unix)]
    pub fn file_type(&self) -> FileType {
        self.0.file_type()
    }
}

impl std::fmt::Debug for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("Metadata");
        // debug.field("file_type", &self.file_type());
        debug.field("permissions", &self.permissions());
        debug.field("len", &self.len());
        if let Ok(modified) = self.modified() {
            debug.field("modified", &modified);
        }
        if let Ok(accessed) = self.accessed() {
            debug.field("accessed", &accessed);
        }
        if let Ok(created) = self.created() {
            debug.field("created", &created);
        }
        debug.finish_non_exhaustive()
    }
}

#[cfg(unix)]
impl MetadataExt for Metadata {
    fn dev(&self) -> u64 {
        self.0.stat.st_dev
    }

    fn ino(&self) -> u64 {
        self.0.stat.st_ino
    }

    fn mode(&self) -> u32 {
        self.0.stat.st_mode
    }

    fn nlink(&self) -> u64 {
        self.0.stat.st_nlink
    }

    fn uid(&self) -> u32 {
        self.0.stat.st_uid
    }

    fn gid(&self) -> u32 {
        self.0.stat.st_gid
    }

    fn rdev(&self) -> u64 {
        self.0.stat.st_rdev
    }

    fn size(&self) -> u64 {
        self.0.stat.st_size as u64
    }

    fn atime(&self) -> i64 {
        self.0.stat.st_atime
    }

    fn atime_nsec(&self) -> i64 {
        self.0.stat.st_atime_nsec
    }

    fn mtime(&self) -> i64 {
        self.0.stat.st_mtime
    }

    fn mtime_nsec(&self) -> i64 {
        self.0.stat.st_mtime_nsec
    }

    fn ctime(&self) -> i64 {
        self.0.stat.st_ctime
    }

    fn ctime_nsec(&self) -> i64 {
        self.0.stat.st_ctime_nsec
    }

    fn blksize(&self) -> u64 {
        self.0.stat.st_blksize as u64
    }

    fn blocks(&self) -> u64 {
        self.0.stat.st_blocks as u64
    }
}

/// Given a path, query the file system to get information about a file,
/// directory, etc.
///
/// This function will traverse symbolic links to query information about the
/// destination file.
///
/// # Platform-specific behavior
///
/// current implementation is only for Linux.
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * The user lacks permissions to perform `metadata` call on `path`.
///     * execute(search) permission is required on all of the directories in path that lead to the
///       file.
/// * `path` does not exist.
///
/// # Examples
///
/// ```rust,no_run
/// use monoio::fs;
///
/// #[monoio::main]
/// async fn main() -> std::io::Result<()> {
///     let attr = fs::metadata("/some/file/path.txt").await?;
///     // inspect attr ...
///     Ok(())
/// }
/// ```

pub async fn metadata<P: AsRef<Path>>(path: P) -> std::io::Result<Metadata> {
    let flags = libc::AT_STATX_SYNC_AS_STAT;

    let op = Op::statx_using_path(path, flags).unwrap();

    op.statx_result().await.map(FileAttr::from).map(Metadata)
}

/// Query the metadata about a file without following symlinks.
///
/// # Platform-specific behavior
///
/// This function currently corresponds to the `lstat` function on linux
///
/// # Errors
///
/// This function will return an error in the following situations, but is not
/// limited to just these cases:
///
/// * The user lacks permissions to perform `metadata` call on `path`.
///     * execute(search) permission is required on all of the directories in path that lead to the
///       file.
/// * `path` does not exist.
///
/// # Examples
/// ```rust,no_run
/// use monoio::fs;
///
/// #[monoio::main]
/// async fn main() -> std::io::Result<()> {
///     let attr = fs::symlink_metadata("/some/file/path.txt").await?;
///     // inspect attr ...
///     Ok(())
/// }
/// ```

pub async fn symlink_metadata<P: AsRef<Path>>(path: P) -> std::io::Result<Metadata> {
    let flags = libc::AT_STATX_SYNC_AS_STAT | libc::AT_SYMLINK_NOFOLLOW;

    let op = Op::statx_using_path(path, flags).unwrap();

    op.statx_result().await.map(FileAttr::from).map(Metadata)
}
