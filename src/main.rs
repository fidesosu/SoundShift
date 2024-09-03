// Currently only prints the title of the window if the window is:
// 1. focused
// 2. is in the 'programs_to_monitor' HashMap
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use sysinfo::{ProcessExt, System, SystemExt};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

fn get_active_window_title() -> String {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        let mut text: [u16; 512] = [0; 512];
        let len = GetWindowTextW(hwnd, &mut text);
        String::from_utf16_lossy(&text[..len as usize])
    }
}

fn main() {
    let mut system = System::new_all();
    system.refresh_all();

    let mut programs_to_monitor: HashMap<String, f32> = HashMap::new();
    programs_to_monitor.insert("firefox".to_string(), 0.1); // Set Firefox to 10% volume when unfocused

    loop {
        let active_window_title = get_active_window_title();

        // Check if the active window's title matches any program in the list
        for (program_name, _) in &programs_to_monitor {
            if active_window_title.to_lowercase().contains(program_name) {
                println!("{}", active_window_title);
                break;
            }
        }

        thread::sleep(Duration::from_millis(500));
        system.refresh_all(); // Refresh system information periodically
    }
}
