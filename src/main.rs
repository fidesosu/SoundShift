mod config;
mod utils {
    pub mod process_utils;
    pub mod volume_utils;
}

use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{System, SystemExt};
use utils::volume_utils::{get_app_volume, set_app_volume};

fn main() {
    let programs_to_monitor = config::load_programs_from_json("programs.json");

    println!("{:?}", programs_to_monitor);

    let mut system = System::new_all();
    system.refresh_all();

    let mut last_process_id: Option<u32> = None;
    let mut previous_volumes: HashMap<u32, f32> = HashMap::new();
    let mut last_refresh_time = Instant::now();
    let refresh_interval = Duration::from_secs(1);
    let mut target_program_previous_volumes: HashMap<u32, f32> = HashMap::new();

    loop {
        if last_refresh_time.elapsed() >= refresh_interval {
            system.refresh_all();
            last_refresh_time = Instant::now();
        }

        let process_id = utils::process_utils::get_foreground_window_process_id();
        let process_name: Option<String> = utils::process_utils::get_process_name_by_id(process_id);

        if let Some(name) = process_name {
            let mut is_target_program = false;

            for (program_name, _volume) in &programs_to_monitor {
                if name.to_lowercase().contains(program_name) {
                    is_target_program = true;
                    break;
                }
            }

            if is_target_program {
                // Store the current volume of the target program BEFORE any changes.
                if !target_program_previous_volumes.contains_key(&process_id) {
                    if let Ok(current_volume) = get_app_volume(process_id) {
                        target_program_previous_volumes.insert(process_id, current_volume);
                    }
                }

                // Raise the volume of the target program to its previous value.
                if let Some(prev_vol) = target_program_previous_volumes.get(&process_id) {
                    if let Err(e) = set_app_volume(process_id, *prev_vol) {
                        eprintln!("Failed to raise volume to previous value: {:?}", e);
                    } else {
                        println!("Raised volume for target program: {} to {}", name, prev_vol);
                    }
                } else {
                    //If the target programs previous volume is unknown, set it to 1.0.
                    if let Err(e) = set_app_volume(process_id, 1.0) {
                        eprintln!("Failed to raise volume to 1.0: {:?}", e);
                    } else {
                        println!("Raised volume for target program: {} to 1.0", name);
                    }
                }

                // Restore previous volume of the last focused program
                if let Some(last_pid) = last_process_id {
                    if last_pid != process_id && previous_volumes.contains_key(&last_pid) {
                        if let Some(prev_vol) = previous_volumes.get(&last_pid) {
                            if let Err(e) = set_app_volume(last_pid, *prev_vol) {
                                eprintln!("Failed to revert volume: {:?}", e);
                            } else {
                                println!("Restored volume to previous window id: {}", last_pid);
                            }
                            previous_volumes.remove(&last_pid);
                        }
                    }
                }
            } else {
    // If NOT a target program, lower its volume
    if let Some(last_pid) = last_process_id {
        if !previous_volumes.contains_key(&last_pid) {
            match get_app_volume(last_pid) {
                Ok(current_volume) => {
                    previous_volumes.insert(last_pid, current_volume);
                    println!("Stored previous volume for process ID {}: {}", last_pid, current_volume);
                }
                Err(e) => {
                    eprintln!("Failed to get current volume: {:?}", e);
                }
            }
        }

        println!("Lowering volume for process ID: {}", last_pid);

        // Find the corresponding volume from the JSON config
        let lower_volume_to = if let Some((program_name, volume)) = programs_to_monitor.iter().find(|(program_name, _)| {
            if let Some(process_name) = utils::process_utils::get_process_name_by_id(last_pid) {
                if let Some(program_name_str) = program_name.as_str().to_lowercase().as_str().strip_suffix("\r"){
                    process_name.to_lowercase().contains(program_name_str)
                } else {
                    process_name.to_lowercase().contains(program_name.as_str().to_lowercase().as_str())
                }
            } else {
                false
            }
        }) {
            *volume // Use the volume from the JSON
        } else {
            0.1 // Default volume if not found
        };

        if let Err(e) = set_app_volume(last_pid, lower_volume_to) {
            eprintln!("Failed to lower volume: {:?}", e);
        }
    }
}

            last_process_id = Some(process_id);
        } else {
            println!("No process name found");
        }

        thread::sleep(Duration::from_millis(500));
    }
}