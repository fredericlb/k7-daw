//! sys — two plain questions of Windows: this machine's DNS hostname, as UTF-8, and its physical memory.

use windows::Win32::System::SystemInformation::{
    ComputerNameDnsHostname, GetComputerNameExW, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows::core::PWSTR;

/// The WIDE name, since `gethostname` answers in the ANSI code page and a name like "PC-FRÉDÉRIC" in cp1252 is not UTF-8.
pub fn host_name() -> Option<String> {
    let mut buf = [0u16; 256];
    let mut n = buf.len() as u32;
    // SAFETY: `buf` is `n` wide characters, as the call is told.
    unsafe {
        GetComputerNameExW(
            ComputerNameDnsHostname,
            Some(PWSTR(buf.as_mut_ptr())),
            &mut n,
        )
    }
    .ok()?;
    (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
}

/// Physical memory in GiB, 0 when Windows will not say (left out of the score, not read as slow).
pub fn memory_gb() -> f64 {
    let mut m = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..MEMORYSTATUSEX::default()
    };
    // SAFETY: `dwLength` is set, as the call requires.
    if unsafe { GlobalMemoryStatusEx(&mut m) }.is_err() {
        return 0.0;
    }
    m.ullTotalPhys as f64 / (1024.0 * 1024.0 * 1024.0)
}
