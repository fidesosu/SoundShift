use serde_json::from_reader;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub fn load_programs_from_json(file_path: &str) -> HashMap<String, f32> {
    if !Path::new(file_path).exists() {
        // Create the JSON file with default content if it doesn't exist
        let default_content = "{  \n\"firefox\": 0.1,   \n\"chrome\": 0.2\n}";
        let file = File::create(file_path).expect("Unable to create JSON file");
        let mut writer = BufWriter::new(file);
        writeln!(writer, "{}", default_content).expect("Unable to write to JSON file");
        println!("Created default JSON file: {}", file_path);

        // Deserialize the default content into a HashMap
        let default_json: HashMap<String, f32> =
            serde_json::from_str(default_content).expect("Unable to parse default JSON");
        default_json
    } else {
        // Open the JSON file and deserialize its contents
        let file = File::open(file_path).expect("Unable to open JSON file");
        let programs: HashMap<String, f32> = from_reader(file).expect("Unable to parse JSON");
        programs
    }
}
