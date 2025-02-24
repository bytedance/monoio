use std::{ffi::CString, io, path::Path};

#[allow(unused_variables)]
pub(super) fn cstr(p: &Path) -> io::Result<CString> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(CString::new(p.as_os_str().as_bytes())?)
    }
    #[cfg(windows)]
    if let Some(s) = p.as_os_str().to_str() {
        Ok(CString::new(s)?)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid utf-8: corrupt contents",
        ))
    }
}

// Convert Duration to Timespec
// It's strange that io_uring does not impl From<Duration> for Timespec.
#[cfg(all(target_os = "linux", feature = "iouring"))]
pub(super) fn timespec(duration: std::time::Duration) -> io_uring::types::Timespec {
    io_uring::types::Timespec::new()
        .sec(duration.as_secs())
        .nsec(duration.subsec_nanos())
}

/// Do syscall and return Result<T, std::io::Error>
/// If use syscall@FD or syscall@NON_FD, the return value is wrapped in MaybeFd. The `MaybeFd` is
/// designed to close the fd when it is dropped.
/// If use syscall@RAW, the return value is raw value. The requirement to explicitly add @RAW is to
/// avoid misuse.
#[cfg(unix)]
#[macro_export]
macro_rules! syscall {
    ($fn: ident @FD ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(unsafe { $crate::driver::op::MaybeFd::new_fd(res as u32) })
        }
    }};
    ($fn: ident @NON_FD ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok($crate::driver::op::MaybeFd::new_non_fd(res as u32))
        }
    }};
    ($fn: ident @RAW ( $($arg: expr),* $(,)* ) ) => {{
        let res = unsafe { libc::$fn($($arg, )*) };
        if res == -1 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res)
        }
    }};
}

/// Do syscall and return Result<T, std::io::Error>
/// If use syscall@FD or syscall@NON_FD, the return value is wrapped in MaybeFd. The `MaybeFd` is
/// designed to close the fd when it is dropped.
/// If use syscall@RAW, the return value is raw value. The requirement to explicitly add @RAW is to
/// avoid misuse.
#[cfg(windows)]
#[macro_export]
macro_rules! syscall {
    ($fn: ident @FD ( $($arg: expr),* $(,)* ), $err_test: path, $err_value: expr) => {{
        let res = unsafe { $fn($($arg, )*) };
        if $err_test(&res, &$err_value) {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(unsafe { $crate::driver::op::MaybeFd::new_fd(res.try_into().unwrap()) })
        }
    }};
    ($fn: ident @NON_FD ( $($arg: expr),* $(,)* ), $err_test: path, $err_value: expr) => {{
        let res = unsafe { $fn($($arg, )*) };
        if $err_test(&res, &$err_value) {
            Err(std::io::Error::last_os_error())
        } else {
            Ok($crate::driver::op::MaybeFd::new_non_fd(res.try_into().unwrap()))
        }
    }};
    ($fn: ident @RAW ( $($arg: expr),* $(,)* ), $err_test: path, $err_value: expr) => {{
        let res = unsafe { $fn($($arg, )*) };
        if $err_test(&res, &$err_value) {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(res.try_into().unwrap())
        }
    }};
}

#[cfg(all(
    not(all(target_os = "linux", feature = "iouring")),
    not(feature = "legacy")
))]
pub(crate) fn feature_panic() -> ! {
    panic!("one of iouring and legacy features must be enabled");
}

#[cfg(all(windows, feature = "renameat"))]
pub(crate) fn to_wide_string(str: &str) -> Vec<u16> {
    use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt};

    OsStr::new(str).encode_wide().chain(once(0)).collect()
}
