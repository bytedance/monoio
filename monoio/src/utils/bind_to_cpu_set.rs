#[cfg(feature = "utils")]
type BindError<T> = nix::Result<T>;

/// Bind current thread to given cpus
#[cfg(feature = "utils")]
pub fn bind_to_cpu_set(cpus: impl IntoIterator<Item = usize>) -> BindError<()> {
    let mut cpuset = nix::sched::CpuSet::new();
    for cpu in cpus {
        cpuset.set(cpu)?;
    }
    let pid = nix::unistd::Pid::from_raw(0);
    nix::sched::sched_setaffinity(pid, &cpuset)
}
