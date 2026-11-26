//! job — the kill-on-close job the HOST holds. The supervisor makes a job, hands a copy of its handle to the
//! host (`$STUDIO_HOST_PID`) and joins it: every process started afterwards, the engine included, dies when the
//! host's handle closes, which is when the host does. What a reader gets wrong: the host's copy is made BEFORE
//! ours is closed, since closing the last handle of a kill-on-close job ends the job and us with it.

use windows::Win32::Foundation::{CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE};

/// A handle closed on drop.
struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: the handle is ours and is closed once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// Joins the host's job when `$STUDIO_HOST_PID` names a host; Ok when there is none, Err with why it could not.
pub fn join_host_job() -> Result<(), String> {
    let Some(pid) = std::env::var("STUDIO_HOST_PID")
        .ok()
        .filter(|v| !v.is_empty())
    else {
        return Ok(());
    };
    let pid: u32 = pid
        .parse()
        .ok()
        .filter(|p| *p != 0)
        .ok_or("STUDIO_HOST_PID is not a pid")?;
    // SAFETY: plain calls on handles this function owns and closes.
    unsafe {
        let host = OpenProcess(PROCESS_DUP_HANDLE, false, pid)
            .map(Owned)
            .map_err(|e| format!("cannot open the host process: {e}"))?;
        let job = CreateJobObjectW(None, None)
            .map(Owned)
            .map_err(|e| format!("CreateJobObject failed with {e}"))?;
        // Kill-on-close and no breakaway: every process we start joins it, the engine included.
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const info).cast(),
            std::mem::size_of_val(&info) as u32,
        )
        .map_err(|e| format!("cannot make the job kill on close: {e}"))?;
        // The host's copy FIRST: closing ours before it exists would end the job, and us with it.
        let mut in_host = HANDLE::default();
        DuplicateHandle(
            GetCurrentProcess(),
            job.0,
            host.0,
            &mut in_host,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
        .map_err(|e| format!("cannot hand the job to the host: {e}"))?;
        AssignProcessToJobObject(job.0, GetCurrentProcess())
            .map_err(|e| format!("cannot join the job: {e}"))
        // Only the host's handle remains once `job` drops, so the job lives exactly as long as the host.
    }
}

/// The app's stdio never into a child: a copy of its pipe outliving us would cost the app our EOF.
pub fn keep_stdio_from_children() {
    use windows::Win32::Foundation::{HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation};
    use windows::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    for id in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: a standard handle is the process's own; a missing one makes the call fail and nothing more.
        unsafe {
            if let Ok(h) = GetStdHandle(id) {
                let _ = SetHandleInformation(h, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0));
            }
        }
    }
}
