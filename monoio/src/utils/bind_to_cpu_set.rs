/// Bind error
pub type BindError<T> = nix::Result<T>;

/// Bind current thread to given cpus
pub fn bind_to_cpu_set(cpus: impl IntoIterator<Item = usize>) -> BindError<()> {
    let mut cpuset = nix::sched::CpuSet::new();
    for cpu in cpus {
        cpuset.set(cpu)?;
    }
    let pid = nix::unistd::Pid::from_raw(0);
    nix::sched::sched_setaffinity(pid, &cpuset)
}

#[cfg(all(test, feature = "utils"))]
mod tests {
    use super::*;

    #[test]
    fn bind_cpu() {
        assert!(bind_to_cpu_set(Some(0)).is_ok());
        assert!(bind_to_cpu_set(Some(100000)).is_err());
    }
}
