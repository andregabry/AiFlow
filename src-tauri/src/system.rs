use crate::model::Result;
// Dedicated OS thread: Windows sleep assertions are thread-scoped.
pub struct AwakeGuard(Option<std::sync::mpsc::Sender<()>>);
impl AwakeGuard {
    pub fn running() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            #[cfg(windows)]
            unsafe {
                windows_sys::Win32::System::Power::SetThreadExecutionState(0x80000001);
            }
            #[cfg(target_os = "macos")]
            let mut inhibitor = std::process::Command::new("/usr/bin/caffeinate")
                .args(["-i", "-w", &std::process::id().to_string()])
                .spawn()
                .ok();
            let _ = rx.recv();
            #[cfg(windows)]
            unsafe {
                windows_sys::Win32::System::Power::SetThreadExecutionState(0x80000000);
            }
            #[cfg(target_os = "macos")]
            if let Some(child) = inhibitor.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        });
        Self(Some(tx))
    }
}
impl Drop for AwakeGuard {
    fn drop(&mut self) {
        self.0.take();
    }
}

#[cfg(windows)]
pub struct ProcessJob(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
unsafe impl Send for ProcessJob {}
#[cfg(windows)]
impl ProcessJob {
    pub fn attach(pid: u32) -> Result<Self> {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::{JobObjects::*, Threading::*},
        };
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err("Cannot create process containment job".into());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of_val(&info) as u32,
            ) == 0
            {
                CloseHandle(job);
                return Err("Cannot configure process containment".into());
            }
            let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false as i32, pid);
            if process.is_null() {
                CloseHandle(job);
                return Err("Cannot open provider process for containment".into());
            }
            let assigned = AssignProcessToJobObject(job, process);
            CloseHandle(process);
            if assigned == 0 {
                CloseHandle(job);
                return Err("Cannot contain provider process; execution stopped".into());
            }
            Ok(Self(job))
        }
    }
}
#[cfg(windows)]
impl Drop for ProcessJob {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
