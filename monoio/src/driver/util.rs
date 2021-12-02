use io_uring::types::Timespec;
use std::ffi::CString;
use std::io;
use std::path::Path;
use std::time::Duration;

pub(super) fn cstr(p: &Path) -> io::Result<CString> {
    use std::os::unix::ffi::OsStrExt;
    Ok(CString::new(p.as_os_str().as_bytes())?)
}

// Convert Duration to Timespec
// It's strange that io_uring does not impl From<Duration> for Timespec.
pub(super) fn timespec(duration: Duration) -> Timespec {
    Timespec::new()
        .sec(duration.as_secs())
        .nsec(duration.subsec_nanos())
}
