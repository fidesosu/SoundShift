use sysinfo::{ProcessExt, System, SystemExt};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

pub fn get_foreground_window_process_id() -> u32 {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        let mut process_id: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process_id as *mut u32));
        process_id
    }
}

pub fn get_process_name_by_id(pid: u32) -> Option<String> {
    let sys = System::new_all();
    let pid = sysinfo::Pid::from(pid as usize);
    if let Some(process) = sys.process(pid) {
        return Some(process.name().to_string());
    }
    None
}
