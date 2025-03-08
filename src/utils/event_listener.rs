use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{EVENT_SYSTEM_FOREGROUND, GetForegroundWindow, GetWindowThreadProcessId};
use windows::Win32::System::Threading::PROCESS_QUERY_INFORMATION;
use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS};
use std::ptr::null_mut;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::thread;
use crate::config;
use crate::utils::volume_utils::{set_app_volume, get_app_volume};

// Define the missing constant
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;

pub fn get_process_name_by_id(process_id: u32) -> Option<String> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32::default();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32>() as u32;

        if Process32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32ProcessID == process_id {
                    // Convert the i8 array to a u16 array
                    let name_u16: Vec<u16> = entry.szExeFile.iter().map(|&c| c as u16).collect();
                    let name = String::from_utf16_lossy(&name_u16);
                    return Some(name);
                }
                if Process32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
    }
    None
}

extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    // This function will be called by the closure with the necessary data
}

pub struct EventListenerData {
    last_process_id: Arc<Mutex<Option<u32>>>,
    previous_volumes: Arc<Mutex<HashMap<u32, f32>>>,
}

pub fn start_event_listener(
    last_process_id: Arc<Mutex<Option<u32>>>,
    previous_volumes: Arc<Mutex<HashMap<u32, f32>>>,
) -> Result<(), windows::core::Error> {
    let data = Arc::new(EventListenerData {
        last_process_id,
        previous_volumes,
    });

    let hook = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };

    if hook.0.is_null() {
        return Err(windows::core::Error::from_win32());
    }

    let data_clone = Arc::clone(&data);
    thread::spawn(move || {
        loop {
            let hwnd = unsafe { GetForegroundWindow() };
            let mut process_id = 0;
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id as *mut _)) };
            let process_name = get_process_name_by_id(process_id);
            println!("Foreground process ID: {}, name: {:?}", process_id, process_name);

            // Load programs to monitor from the JSON file, creating it if necessary
            let programs_to_monitor = config::load_programs_from_json("programs.json");

            if let Some(name) = process_name {
                let mut is_target_program = false;

                // Check if the process name matches any program in the list
                for (program_name, volume) in &programs_to_monitor {
                    if name.to_lowercase().contains(program_name) {
                        is_target_program = true;
                        println!("Target program '{}' is focused", program_name);

                        // Revert the volume if the target program is focused
                        if let Some(last_pid) = *data_clone.last_process_id.lock().unwrap() {
                            println!("Last process ID: {}", last_pid);
                            if let Some(prev_vol) = data_clone.previous_volumes.lock().unwrap().get(&process_id) {
                                println!(
                                    "Found previous volume for process ID {}: {}",
                                    last_pid, prev_vol
                                );
                                println!("Reverting volume for process ID: {}", process_id);
                                if let Err(e) = set_app_volume(process_id, *prev_vol) {
                                    eprintln!(
                                        "Failed to revert volume for process ID {}: {:?}",
                                        process_id, e
                                    );
                                } else {
                                    println!(
                                        "Successfully reverted volume for process ID: {}",
                                        process_id
                                    );
                                }
                                data_clone.previous_volumes.lock().unwrap().remove(&process_id);
                            } else {
                                println!(
                                    "No previous volume stored to revert for process ID: {}",
                                    process_id
                                );
                            }
                        } else {
                            println!("No last process ID to revert volume for");
                        }

                        break;
                    }
                }

                // Lower the volume if the target program is not focused
                if !is_target_program {
                    if let Some(last_pid) = *data_clone.last_process_id.lock().unwrap() {
                        if !data_clone.previous_volumes.lock().unwrap().contains_key(&last_pid) {
                            // Retrieve and store the current volume before lowering it
                            match get_app_volume(last_pid) {
                                Ok(current_volume) => {
                                    data_clone.previous_volumes.lock().unwrap().insert(last_pid, current_volume);
                                    println!("Stored previous volume for process ID {}: {}", last_pid, current_volume);
                                }
                                Err(e) => {
                                    eprintln!("Failed to get current volume for process ID {}: {:?}", last_pid, e);
                                }
                            }
                        }
                        println!("Lowering volume for process ID: {}", last_pid);
                        if let Err(e) = set_app_volume(last_pid, 0.1) { // Example volume level
                            eprintln!("Failed to lower volume for process ID {}: {:?}", last_pid, e);
                        }
                    }
                }

                // Update last_process_id regardless of whether the process is in the list
                *data_clone.last_process_id.lock().unwrap() = Some(process_id);
            } else {
                println!("No process name found for process ID: {}", process_id);
            }

            thread::sleep(std::time::Duration::from_millis(50));
        }
    });

    Ok(())
}