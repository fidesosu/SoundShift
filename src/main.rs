mod process_utils;
mod config;

use std::thread;
use std::time::Duration;
use sysinfo::{ProcessExt, System, SystemExt};

fn main() {
    // Load programs to monitor from the JSON file, creating it if necessary
    let programs_to_monitor = config::load_programs_from_json("programs.json");

    let mut system = System::new_all();
    system.refresh_all();

    loop {
        // Get the process ID of the foreground window
        let process_id = process_utils::get_foreground_window_process_id();
        
        // Refresh system information to get the latest process data
        system.refresh_all();

        // Retrieve the process name
        let process_name = process_utils::get_process_name_by_id(process_id);

        if let Some(name) = process_name {
            // Check if the process name matches any program in the list
            for (program_name, _) in &programs_to_monitor {
                if name.to_lowercase().contains(program_name) {
                    println!("Monitoring process: {}", name);
                    break;
                }
            }
        }

        // Sleep for a bit before the next check
        thread::sleep(Duration::from_millis(500));
    }
}
