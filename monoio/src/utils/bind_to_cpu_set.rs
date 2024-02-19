/// Bind error
#[cfg(unix)]
pub type BindError<T> = nix::Result<T>;

/// Bind error
#[cfg(windows)]
pub type BindError<T> = std::io::Result<T>;

/// Bind current thread to given cpus
#[cfg(any(target_os = "android", target_os = "dragonfly", target_os = "linux"))]
pub fn bind_to_cpu_set(cpus: impl IntoIterator<Item = usize>) -> BindError<()> {
    let mut cpuset = nix::sched::CpuSet::new();
    for cpu in cpus {
        cpuset.set(cpu)?;
    }
    let pid = nix::unistd::Pid::from_raw(0);
    nix::sched::sched_setaffinity(pid, &cpuset)
}

/// Bind current thread to given cpus(but not works for non-linux)
#[cfg(all(
    unix,
    not(any(target_os = "android", target_os = "dragonfly", target_os = "linux"))
))]
pub fn bind_to_cpu_set(_: impl IntoIterator<Item = usize>) -> BindError<()> {
    Ok(())
}

/// Bind current thread to given cpus
#[cfg(windows)]
pub fn bind_to_cpu_set(_: impl IntoIterator<Item = usize>) -> BindError<()> {
    Ok(())
}

#[cfg(all(test, feature = "utils"))]
mod tests {
    use super::*;

    #[test]
    fn bind_cpu() {
        assert!(bind_to_cpu_set(Some(0)).is_ok());
        #[cfg(all(
            unix,
            any(target_os = "android", target_os = "dragonfly", target_os = "linux")
        ))]
        assert!(bind_to_cpu_set(Some(100000)).is_err());
    }
}
