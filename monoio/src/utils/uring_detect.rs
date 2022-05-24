//! Detect if current platform support io_uring.

#[cfg(target_os = "linux")]
macro_rules! op_codes {
    ($($op: ident),*) => {
        [$(io_uring::opcode::$op::CODE),*]
    };
}

#[cfg(target_os = "linux")]
macro_rules! err_to_false {
    ($e: expr) => {
        match $e {
            Ok(x) => x,
            Err(_) => {
                return false;
            }
        }
    };
}

#[cfg(target_os = "linux")]
fn detect_uring_inner() -> bool {
    const USED_OP: [u8; 14] = op_codes![
        Accept,
        AsyncCancel,
        Close,
        Connect,
        Fsync,
        OpenAt,
        ProvideBuffers,
        Read,
        Readv,
        Recv,
        Send,
        Timeout,
        Write,
        Writev
    ];

    let uring = err_to_false!(io_uring::IoUring::new(2));
    let mut probe = io_uring::Probe::new();
    err_to_false!(uring.submitter().register_probe(&mut probe));
    USED_OP.iter().all(|op| probe.is_supported(*op))
}

/// Detect if current platform supports our needed uring ops.
#[cfg(target_os = "linux")]
pub fn detect_uring() -> bool {
    static mut URING_SUPPORTED: bool = false;
    static INIT: std::sync::Once = std::sync::Once::new();

    unsafe {
        INIT.call_once(|| {
            URING_SUPPORTED = detect_uring_inner();
        });
        URING_SUPPORTED
    }
}

/// Detect if current platform supports our needed uring ops.
#[cfg(not(target_os = "linux"))]
pub fn detect_uring() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn test_detect() {
        assert!(
            super::detect_uring(),
            "io_uring or ops not supported on current platform"
        )
    }
}
