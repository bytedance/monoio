#[cfg(unix)]
use std::os::unix::prelude::OpenOptionsExt;
use std::{io, path::Path};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{ERROR_INVALID_PARAMETER, GENERIC_READ, GENERIC_WRITE},
    Security::SECURITY_ATTRIBUTES,
    Storage::FileSystem::{
        CREATE_ALWAYS, CREATE_NEW, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_WRITE,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_WRITE_DATA, OPEN_ALWAYS,
        OPEN_EXISTING, TRUNCATE_EXISTING,
    },
};

use crate::{
    driver::{op::Op, shared_fd::SharedFd},
    fs::File,
};

/// Options and flags which can be used to configure how a file is opened.
///
/// This builder exposes the ability to configure how a [`File`] is opened and
/// what operations are permitted on the open file. The [`File::open`] and
/// [`File::create`] methods are aliases for commonly used options using this
/// builder.
///
/// Generally speaking, when using `OpenOptions`, you'll first call
/// [`OpenOptions::new`], then chain calls to methods to set each option, then
/// call [`OpenOptions::open`], passing the path of the file you're trying to
/// open. This will give you a [`io::Result`] with a [`File`] inside that you
/// can further operate on.
///
/// # Examples
///
/// Opening a file to read:
///
/// ```no_run
/// use monoio::fs::OpenOptions;
///
/// #[monoio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let file = OpenOptions::new().read(true).open("foo.txt").await?;
///     Ok(())
/// }
/// ```
///
/// Opening a file for both reading and writing, as well as creating it if it
/// doesn't exist:
///
/// ```no_run
/// use monoio::fs::OpenOptions;
///
/// #[monoio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let file = OpenOptions::new()
///         .read(true)
///         .write(true)
///         .create(true)
///         .open("foo.txt")
///         .await?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
    #[cfg(unix)]
    pub(crate) mode: libc::mode_t,
    #[cfg(unix)]
    pub(crate) custom_flags: libc::c_int,
    #[cfg(windows)]
    pub(crate) custom_flags: u32,
    #[cfg(windows)]
    pub(crate) access_mode: Option<u32>,
    #[cfg(windows)]
    pub(crate) attributes: u32,
    #[cfg(windows)]
    pub(crate) share_mode: u32,
    #[cfg(windows)]
    pub(crate) security_qos_flags: u32,
    #[cfg(windows)]
    pub(crate) security_attributes: *mut SECURITY_ATTRIBUTES,
}

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    ///
    /// All options are initially set to `false`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs::OpenOptions;
    ///
    /// #[monoio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let file = OpenOptions::new().read(true).open("foo.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    #[allow(clippy::new_without_default)]
    pub fn new() -> OpenOptions {
        OpenOptions {
            // generic
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
            #[cfg(unix)]
            mode: 0o666,
            #[cfg(unix)]
            custom_flags: 0,
            #[cfg(windows)]
            custom_flags: 0,
            #[cfg(windows)]
            access_mode: None,
            #[cfg(windows)]
            share_mode: FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            #[cfg(windows)]
            attributes: 0,
            #[cfg(windows)]
            security_qos_flags: 0,
            #[cfg(windows)]
            security_attributes: std::ptr::null_mut(),
        }
    }

    /// Sets the option for read access.
    ///
    /// This option, when true, will indicate that the file should be
    /// `read`-able if opened.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs::OpenOptions;
    ///
    /// #[monoio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let file = OpenOptions::new().read(true).open("foo.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn read(&mut self, read: bool) -> &mut OpenOptions {
        self.read = read;
        self
    }

    /// Sets the option for write access.
    ///
    /// This option, when true, will indicate that the file should be
    /// `write`-able if opened.
    ///
    /// If the file already exists, any write calls on it will overwrite its
    /// contents, without truncating it.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs::OpenOptions;
    ///
    /// #[monoio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let file = OpenOptions::new().write(true).open("foo.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn write(&mut self, write: bool) -> &mut OpenOptions {
        self.write = write;
        self
    }

    /// Sets the option for the append mode.
    ///
    /// This option, when true, means that writes will append to a file instead
    /// of overwriting previous contents. Note that setting
    /// `.write(true).append(true)` has the same effect as setting only
    /// `.append(true)`.
    ///
    /// For most filesystems, the operating system guarantees that all writes
    /// are atomic: no writes get mangled because another process writes at the
    /// same time.
    ///
    /// ## Note
    ///
    /// This function doesn't create the file if it doesn't exist. Use the
    /// [`OpenOptions::create`] method to do so.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs::OpenOptions;
    ///
    /// #[monoio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let file = OpenOptions::new().append(true).open("foo.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn append(&mut self, append: bool) -> &mut OpenOptions {
        self.append = append;
        self
    }

    /// Sets the option for truncating a previous file.
    ///
    /// If a file is successfully opened with this option set it will truncate
    /// the file to 0 length if it already exists.
    ///
    /// The file must be opened with write access for truncate to work.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs::OpenOptions;
    ///
    /// #[monoio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let file = OpenOptions::new()
    ///         .write(true)
    ///         .truncate(true)
    ///         .open("foo.txt")
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn truncate(&mut self, truncate: bool) -> &mut OpenOptions {
        self.truncate = truncate;
        self
    }

    /// Sets the option to create a new file, or open it if it already exists.
    ///
    /// In order for the file to be created, [`OpenOptions::write`] or
    /// [`OpenOptions::append`] access must be used.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs::OpenOptions;
    ///
    /// #[monoio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let file = OpenOptions::new()
    ///         .write(true)
    ///         .create(true)
    ///         .open("foo.txt")
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn create(&mut self, create: bool) -> &mut OpenOptions {
        self.create = create;
        self
    }

    /// Sets the option to create a new file, failing if it already exists.
    ///
    /// No file is allowed to exist at the target location, also no (dangling)
    /// symlink. In this way, if the call succeeds, the file returned is
    /// guaranteed to be new.
    ///
    /// This option is useful because it is atomic. Otherwise between checking
    /// whether a file exists and creating a new one, the file may have been
    /// created by another process (a TOCTOU race condition / attack).
    ///
    /// If `.create_new(true)` is set, [`.create()`] and [`.truncate()`] are
    /// ignored.
    ///
    /// The file must be opened with write or append access in order to create
    /// a new file.
    ///
    /// [`.create()`]: OpenOptions::create
    /// [`.truncate()`]: OpenOptions::truncate
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs::OpenOptions;
    ///
    /// #[monoio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let file = OpenOptions::new()
    ///         .write(true)
    ///         .create_new(true)
    ///         .open("foo.txt")
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub fn create_new(&mut self, create_new: bool) -> &mut OpenOptions {
        self.create_new = create_new;
        self
    }

    /// Opens a file at `path` with the options specified by `self`.
    ///
    /// # Errors
    ///
    /// This function will return an error under a number of different
    /// circumstances. Some of these error conditions are listed here, together
    /// with their [`io::ErrorKind`]. The mapping to [`io::ErrorKind`]s is not
    /// part of the compatibility contract of the function, especially the
    /// [`Other`] kind might change to more specific kinds in the future.
    ///
    /// * [`NotFound`]: The specified file does not exist and neither `create` or `create_new` is
    ///   set.
    /// * [`NotFound`]: One of the directory components of the file path does not exist.
    /// * [`PermissionDenied`]: The user lacks permission to get the specified access rights for the
    ///   file.
    /// * [`PermissionDenied`]: The user lacks permission to open one of the directory components of
    ///   the specified path.
    /// * [`AlreadyExists`]: `create_new` was specified and the file already exists.
    /// * [`InvalidInput`]: Invalid combinations of open options (truncate without write access, no
    ///   access mode set, etc.).
    /// * [`Other`]: One of the directory components of the specified file path was not, in fact, a
    ///   directory.
    /// * [`Other`]: Filesystem-level errors: full disk, write permission requested on a read-only
    ///   file system, exceeded disk quota, too many open files, too long filename, too many
    ///   symbolic links in the specified path (Unix-like systems only), etc.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use monoio::fs::OpenOptions;
    ///
    /// #[monoio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let file = OpenOptions::new().read(true).open("foo.txt").await?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`AlreadyExists`]: io::ErrorKind::AlreadyExists
    /// [`InvalidInput`]: io::ErrorKind::InvalidInput
    /// [`NotFound`]: io::ErrorKind::NotFound
    /// [`Other`]: io::ErrorKind::Other
    /// [`PermissionDenied`]: io::ErrorKind::PermissionDenied
    pub async fn open(&self, path: impl AsRef<Path>) -> io::Result<File> {
        let op = Op::open(path.as_ref(), self)?;

        // Await the completion of the event
        let completion = op.await;

        // The file is open
        Ok(File::from_shared_fd(SharedFd::new_without_register(
            completion.meta.result? as _,
        )))
    }

    #[cfg(unix)]
    pub(crate) fn access_mode(&self) -> io::Result<libc::c_int> {
        match (self.read, self.write, self.append) {
            (true, false, false) => Ok(libc::O_RDONLY),
            (false, true, false) => Ok(libc::O_WRONLY),
            (true, true, false) => Ok(libc::O_RDWR),
            (false, _, true) => Ok(libc::O_WRONLY | libc::O_APPEND),
            (true, _, true) => Ok(libc::O_RDWR | libc::O_APPEND),
            (false, false, false) => Err(io::Error::from_raw_os_error(libc::EINVAL)),
        }
    }

    #[cfg(windows)]
    pub(crate) fn access_mode(&self) -> io::Result<u32> {
        match (self.read, self.write, self.append, self.access_mode) {
            (.., Some(mode)) => Ok(mode),
            (true, false, false, None) => Ok(GENERIC_READ),
            (false, true, false, None) => Ok(GENERIC_WRITE),
            (true, true, false, None) => Ok(GENERIC_READ | GENERIC_WRITE),
            (false, _, true, None) => Ok(FILE_GENERIC_WRITE & !FILE_WRITE_DATA),
            (true, _, true, None) => Ok(GENERIC_READ | (FILE_GENERIC_WRITE & !FILE_WRITE_DATA)),
            (false, false, false, None) => {
                Err(io::Error::from_raw_os_error(ERROR_INVALID_PARAMETER))
            }
        }
    }

    #[cfg(unix)]
    pub(crate) fn creation_mode(&self) -> io::Result<libc::c_int> {
        match (self.write, self.append) {
            (true, false) => {}
            (false, false) => {
                if self.truncate || self.create || self.create_new {
                    return Err(io::Error::from_raw_os_error(libc::EINVAL));
                }
            }
            (_, true) => {
                if self.truncate && !self.create_new {
                    return Err(io::Error::from_raw_os_error(libc::EINVAL));
                }
            }
        }

        Ok(match (self.create, self.truncate, self.create_new) {
            (false, false, false) => 0,
            (true, false, false) => libc::O_CREAT,
            (false, true, false) => libc::O_TRUNC,
            (true, true, false) => libc::O_CREAT | libc::O_TRUNC,
            (_, _, true) => libc::O_CREAT | libc::O_EXCL,
        })
    }

    #[cfg(windows)]
    pub(crate) fn creation_mode(&self) -> io::Result<u32> {
        match (self.write, self.append) {
            (true, false) => {}
            (false, false) => {
                if self.truncate || self.create || self.create_new {
                    return Err(io::Error::from_raw_os_error(ERROR_INVALID_PARAMETER));
                }
            }
            (_, true) => {
                if self.truncate && !self.create_new {
                    return Err(io::Error::from_raw_os_error(ERROR_INVALID_PARAMETER));
                }
            }
        }

        Ok(match (self.create, self.truncate, self.create_new) {
            (false, false, false) => OPEN_EXISTING,
            (true, false, false) => OPEN_ALWAYS,
            (false, true, false) => TRUNCATE_EXISTING,
            (true, true, false) => CREATE_ALWAYS,
            (_, _, true) => CREATE_NEW,
        })
    }

    #[cfg(windows)]
    pub(crate) fn get_flags_and_attributes(&self) -> u32 {
        self.custom_flags
            | self.attributes
            | self.security_qos_flags
            | if self.create_new {
                FILE_FLAG_OPEN_REPARSE_POINT
            } else {
                0
            }
    }
}

#[cfg(unix)]
impl OpenOptionsExt for OpenOptions {
    fn mode(&mut self, mode: u32) -> &mut Self {
        self.mode = mode as libc::mode_t;
        self
    }

    fn custom_flags(&mut self, flags: i32) -> &mut Self {
        self.custom_flags = flags as libc::c_int;
        self
    }
}
