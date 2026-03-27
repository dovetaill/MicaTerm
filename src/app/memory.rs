//! Platform helpers for trimming retained process memory after large idle output bursts.

#[cfg(windows)]
pub fn trim_process_working_set() {
    use windows_sys::Win32::System::ProcessStatus::K32EmptyWorkingSet;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // SAFETY: GetCurrentProcess returns a pseudo-handle for the current process,
    // which is valid for the duration of this call.
    let trimmed = unsafe { K32EmptyWorkingSet(GetCurrentProcess()) };
    if trimmed == 0 {
        tracing::debug!(target: "app.memory", "working set trim request was ignored");
    }
}

#[cfg(not(windows))]
pub fn trim_process_working_set() {}
