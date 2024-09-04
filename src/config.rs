use serde::Deserialize;
use serde_json::{from_reader, Value};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

// Define a struct for deserializing the JSON data
#[derive(Deserialize)]
struct ProgramsToMonitor {
    programs: HashMap<String, f32>,
}

pub fn load_programs_from_json(file_path: &str) -> HashMap<String, f32> {
    if !Path::new(file_path).exists() {
        // Create the JSON file with default content if it doesn't exist
        let default_content = r#"{
            "programs": {
                "firefox": 0.1,
                "chrome": 0.2
            }
        }"#;
        let file = File::create(file_path).expect("Unable to create JSON file");
        let mut writer = BufWriter::new(file);
        writeln!(writer, "{}", default_content).expect("Unable to write to JSON file");
        println!("Created default JSON file: {}", file_path);

        // Deserialize the default content into a HashMap
        let default_json: ProgramsToMonitor = serde_json::from_str(default_content).expect("Unable to parse default JSON");
        default_json.programs
    } else {
        // Open the JSON file and deserialize its contents
        let file = File::open(file_path).expect("Unable to open JSON file");
        let programs: ProgramsToMonitor = from_reader(file).expect("Unable to parse JSON");
        programs.programs
    }
}
