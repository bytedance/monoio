use std::{fmt::Debug, os::unix::fs::PermissionsExt};

#[cfg(unix)]
use libc::mode_t;

#[cfg(unix)]
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FilePermissions {
    pub(crate) mode: mode_t,
}

impl FilePermissions {
    fn readonly(&self) -> bool {
        self.mode & 0o222 == 0
    }

    #[cfg(target_os = "linux")]
    fn mode(&self) -> u32 {
        self.mode
    }

    #[cfg(not(target_os = "linux"))]
    fn mode(&self) -> u32 {
        unimplemented!()
    }
}

/// Representation of the various permissions on a file.
#[cfg(unix)]
pub struct Permissions(pub(crate) FilePermissions);

impl Permissions {
    /// Returns `true` if these permissions describe a readonly (unwritable) file.
    pub fn readonly(&self) -> bool {
        self.0.readonly()
    }

    /// Set the readonly flag for this set of permissions.
    ///
    /// # NOTE
    /// this function is unimplemented because current don't know how to sync the
    /// mode bits to the file. So currently, it will not expose to the user.
    #[allow(unused)]
    pub(crate) fn set_readonly(&self, _read_only: bool) {
        unimplemented!()
    }
}

impl PermissionsExt for Permissions {
    /// Returns the underlying raw `mode_t` bits that are used by the OS.
    fn mode(&self) -> u32 {
        self.0.mode()
    }

    /// Set the mode bits for this set of permissions.
    ///
    /// this function is unimplemented because current don't know how to sync the
    /// mode bits to the file.
    fn set_mode(&mut self, _mode: u32) {
        unimplemented!()
    }

    /// Create a new instance of `Permissions` from the given mode bits.
    fn from_mode(mode: u32) -> Self {
        Self(FilePermissions {
            mode: mode as mode_t,
        })
    }
}

impl Debug for Permissions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Permissions")
            .field("readonly", &self.readonly())
            .finish()
    }
}
