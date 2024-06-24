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

    fn set_readonly(&mut self, readonly: bool) {
        if readonly {
            self.mode &= !0o222;
        } else {
            self.mode |= 0o222;
        }
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
    /// This will not change the file's permissions, only the in-memory representation.
    /// Same with the `std::fs`, if you want to change the file's permissions, you should use
    /// `monoio::fs::set_permissions`(currently not support) or `std::fs::set_permissions`.
    #[allow(unused)]
    pub fn set_readonly(&mut self, readonly: bool) {
        self.0.set_readonly(readonly)
    }
}

impl PermissionsExt for Permissions {
    /// Returns the underlying raw `mode_t` bits that are used by the OS.
    fn mode(&self) -> u32 {
        self.0.mode()
    }

    /// Set the mode bits for this set of permissions.
    ///
    /// This will not change the file's permissions, only the in-memory representation.
    /// Same with the `std::fs`, if you want to change the file's permissions, you should use
    /// `monoio::fs::set_permissions`(currently not support) or `std::fs::set_permissions`.
    fn set_mode(&mut self, mode: u32) {
        *self = Self::from_mode(mode);
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
            .finish_non_exhaustive()
    }
}
