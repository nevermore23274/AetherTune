//! Win32 Job Object wrapper for child process lifecycle management.
//!
//! When AetherTune spawns mpv.exe, we assign it to a Job Object with
//! JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE. This ensures mpv is terminated
//! automatically when AetherTune exits — even on a crash, forced close,
//! or terminal disconnect. Without this, mpv.exe survives as an orphan.

use std::os::windows::io::AsRawHandle;

// ── Raw Win32 FFI (avoids adding windows-sys as a dependency) ──────────

type HANDLE = *mut std::ffi::c_void;
type BOOL = i32;
type DWORD = u32;

const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: DWORD = 0x2000;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION: DWORD = 9;

#[repr(C)]
#[allow(non_snake_case)]
struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
    PerProcessUserTimeLimit: i64,
    PerJobUserTimeLimit: i64,
    LimitFlags: DWORD,
    MinimumWorkingSetSize: usize,
    MaximumWorkingSetSize: usize,
    ActiveProcessLimit: DWORD,
    Affinity: usize,
    PriorityClass: DWORD,
    SchedulingClass: DWORD,
}

#[repr(C)]
#[allow(non_snake_case)]
struct IO_COUNTERS {
    ReadOperationCount: u64,
    WriteOperationCount: u64,
    OtherOperationCount: u64,
    ReadTransferCount: u64,
    WriteTransferCount: u64,
    OtherTransferCount: u64,
}

#[repr(C)]
#[allow(non_snake_case)]
struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION,
    IoInfo: IO_COUNTERS,
    ProcessMemoryLimit: usize,
    JobMemoryLimit: usize,
    PeakProcessMemoryUsed: usize,
    PeakJobMemoryUsed: usize,
}

unsafe extern "system" {
    fn CreateJobObjectW(
        lpJobAttributes: HANDLE,
        lpName: *const u16,
    ) -> HANDLE;

    fn SetInformationJobObject(
        hJob: HANDLE,
        JobObjectInformationClass: DWORD,
        lpJobObjectInformation: *const std::ffi::c_void,
        cbJobObjectInformationLength: DWORD,
    ) -> BOOL;

    fn AssignProcessToJobObject(
        hJob: HANDLE,
        hProcess: HANDLE,
    ) -> BOOL;

    fn CloseHandle(hObject: HANDLE) -> BOOL;
}

// ── Public API ─────────────────────────────────────────────────────────

/// A Win32 Job Object configured to kill all assigned processes when dropped.
pub struct JobObject {
    handle: HANDLE,
}

// SAFETY: Job Object handles are kernel handles with no thread affinity.
unsafe impl Send for JobObject {}

impl JobObject {
    /// Create a new Job Object with KILL_ON_JOB_CLOSE.
    /// Returns None if the Win32 calls fail (shouldn't happen on any
    /// supported Windows version).
    pub fn new() -> Option<Self> {
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            if handle.is_null() {
                return None;
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let result = SetInformationJobObject(
                handle,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as DWORD,
            );

            if result == 0 {
                CloseHandle(handle);
                return None;
            }

            Some(JobObject { handle })
        }
    }

    /// Assign a child process to this Job Object.
    /// Once assigned, the process will be killed when this JobObject is dropped.
    pub fn assign(&self, child: &std::process::Child) {
        unsafe {
            let process_handle = child.as_raw_handle() as HANDLE;
            AssignProcessToJobObject(self.handle, process_handle);
        }
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        unsafe {
            // Closing the handle with KILL_ON_JOB_CLOSE kills all assigned processes
            CloseHandle(self.handle);
        }
    }
}