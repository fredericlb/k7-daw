// job.rs — the app in a Windows Job Object of its own, kill-on-close: whatever it starts, the front's children
// included (the hub's services, renders, the supervisor), dies with it, however it ends. Membership is inherited.
// What a reader gets wrong: what must outlive the app (the browser, a restart, the installer) leaves first — docs/app.md.

#[cfg(windows)]
mod imp {
    use std::sync::OnceLock;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT,
        JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows::Win32::System::Threading::GetCurrentProcess;

    /// `CREATE_BREAKAWAY_FROM_JOB`: a child started with it is in no job of the app's.
    pub const BREAKAWAY: u32 = 0x0100_0000;

    // The job HANDLE as a usize (a raw pointer isn't Send/Sync); 0 means no job. NEVER closed: the OS closing it at
    // the app's exit is the kill.
    static JOB: OnceLock<usize> = OnceLock::new();

    fn limit(job: HANDLE, flags: JOB_OBJECT_LIMIT) -> bool {
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = flags;
        unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .is_ok()
        }
    }

    /// The app joins a kill-on-close job, before it starts anything; a failure is said and leaves the net off.
    pub fn init() -> Result<(), String> {
        let raw = *JOB.get_or_init(|| unsafe {
            let job = match CreateJobObjectW(None, PCWSTR::null()) {
                Ok(h) if !h.is_invalid() => h,
                _ => return 0,
            };
            if !limit(
                job,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK,
            ) {
                return 0;
            }
            if AssignProcessToJobObject(job, GetCurrentProcess()).is_err() {
                return 0;
            }
            job.0 as usize
        });
        if raw == 0 {
            return Err(format!(
                "no kill-on-close job: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn init() -> Result<(), String> {
        Ok(())
    }
}

pub use imp::init;

/// Spawn what must outlive the app (the browser, an editor) out of its job; inside it when the app's own parent forbids leaving.
#[cfg(windows)]
pub fn spawn_outside(c: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    use std::os::windows::process::CommandExt;
    c.creation_flags(0x0800_0000 | imp::BREAKAWAY);
    match c.spawn() {
        Ok(child) => Ok(child),
        Err(_) => c.creation_flags(0x0800_0000).spawn(),
    }
}
