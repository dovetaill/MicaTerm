//! Platform helpers for trimming retained process memory after large idle output bursts.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProcessMemorySnapshot {
    pub working_set_bytes: usize,
    pub peak_working_set_bytes: usize,
    pub pagefile_usage_bytes: usize,
    pub private_usage_bytes: usize,
}

#[cfg(windows)]
pub fn current_process_memory_snapshot() -> Option<ProcessMemorySnapshot> {
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // SAFETY: zeroed is valid for this plain-old-data Win32 struct, and we
    // immediately populate the required `cb` size field before the API call.
    let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    // SAFETY: GetCurrentProcess returns a valid pseudo-handle and the buffer
    // lives for the duration of the call.
    let ok = unsafe {
        K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
    };
    if ok == 0 {
        None
    } else {
        Some(ProcessMemorySnapshot {
            working_set_bytes: counters.WorkingSetSize,
            peak_working_set_bytes: counters.PeakWorkingSetSize,
            pagefile_usage_bytes: counters.PagefileUsage,
            private_usage_bytes: counters.PrivateUsage,
        })
    }
}

#[cfg(not(windows))]
pub fn current_process_memory_snapshot() -> Option<ProcessMemorySnapshot> {
    None
}

#[cfg(windows)]
pub fn trim_process_working_set() -> bool {
    use windows_sys::Win32::System::ProcessStatus::K32EmptyWorkingSet;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // SAFETY: GetCurrentProcess returns a pseudo-handle for the current process,
    // which is valid for the duration of this call.
    let trimmed = unsafe { K32EmptyWorkingSet(GetCurrentProcess()) };
    if trimmed == 0 {
        tracing::debug!(target: "app.memory", "working set trim request was ignored");
        false
    } else {
        true
    }
}

#[cfg(not(windows))]
pub fn trim_process_working_set() -> bool {
    false
}
