use adb_client::{ADBDeviceExt, server::ADBServer};
use console::style;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use serde_json::from_reader;
use std::env;
use std::fs::{File, metadata};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    local_dir: String,
    remote_dir: String,
    allow_hidden: bool,
}

fn main() {
    let mut root_path = match env::home_dir() {
        Some(path) => path,
        None => panic!("No root path found"),
    };

    let config_file = File::open("config.json").expect("Cannot find config file");
    let config: ConfigFile = serde_json::from_reader(config_file).expect("Cannot read config file");

    let local_dir: String = config.local_dir;
    let remote_dir: String = config.remote_dir;
    let allow_hidden: bool = config.allow_hidden;

    let spinner_style = ProgressStyle::with_template("{prefix:.bold.dim} {spinner} {wide_msg}")
        .unwrap()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");

    root_path.push(local_dir);
    let abs_root_path = &root_path.display().to_string();

    let mut server = ADBServer::default();
    let mut device = server.get_device().expect("Can't get device");

    let mut add: u64 = 0;
    let mut change: u64 = 0;

    let mut queue: Vec<PathBuf> = Vec::new();

    // Walk directories recursively and record the file paths of files that need to be added/changed
    // as a vector.
    let directory_loader = ProgressBar::new_spinner();
    println!("Fetching changes...");
    directory_loader.enable_steady_tick(Duration::from_millis(100));

    for entry in WalkDir::new(&root_path) {
        let path = entry.unwrap().path().to_path_buf();

        let rel_path = path
            .strip_prefix(abs_root_path)
            .expect("Error: wrong prefix");

        let mut remote_path = PathBuf::from(&remote_dir);
        remote_path.push(rel_path);
        let remote_path_str = remote_path
            .into_os_string()
            .into_string()
            .expect("Invalid path");

        let modified_time = device.stat(&remote_path_str).unwrap().mod_time as u64;

        let path_metadata = metadata(&path).expect("Path not found");

        if modified_time == 0 {
            if allow_hidden || !&path.file_name().unwrap().to_string_lossy().starts_with('.') {
                if path_metadata.is_file() {
                    add += 1;
                }
                queue.push(path);
            }
        } else if (modified_time
            < path_metadata
                .modified()
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs())
            && (allow_hidden || !&path.file_name().unwrap().to_string_lossy().starts_with('.'))
        {
            if path_metadata.is_file() {
                change += 1;
            }
            queue.push(path);
        }
    }
    directory_loader.finish();

    println!("Files to add: {}", add);
    println!("Files to change: {}", change);
    let confirmation = Confirm::new()
        .with_prompt("Do you want to make changes?")
        .interact()
        .unwrap();

    if !confirmation {
        println!("Exiting...");
    } else {
        let total: u64 = add + change;
        let mut curr_idx: u64 = 1;
        for path in queue {
            let path_metadata = metadata(&path).unwrap();
            let mut remote_path = PathBuf::from(&remote_dir);

            let rel_path = path
                .strip_prefix(abs_root_path)
                .expect("Error: wrong prefix");

            let rel_path_str = rel_path.to_str().unwrap();

            remote_path.push(rel_path);
            let remote_path_str = remote_path
                .into_os_string()
                .into_string()
                .expect("Invalid path");

            let modified_time = device.stat(&remote_path_str).unwrap().mod_time as u64;
            // Since this is unix time, a value of 0 means the file does not exist.
            if modified_time
                < path_metadata
                    .modified()
                    .unwrap()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            {
                if path_metadata.is_file() {
                    let file_path = File::open(&path).expect("File not found!");
                    if allow_hidden
                        || !&path.file_name().unwrap().to_string_lossy().starts_with('.')
                    {
                        let add_or_update: &str = if modified_time == 0 {
                            "Adding"
                        } else {
                            "Updating"
                        };
                        let curr_idx_str = format!("[{}/{}]", curr_idx, total);
                        let push_message = format!("{} {}", add_or_update, rel_path_str);
                        println!("{} {}", style(curr_idx_str).bold().dim(), push_message);
                        device
                            .push(file_path, &remote_path_str)
                            .expect("file push error");
                        curr_idx += 1;
                    }
                } else {
                    let cmd = format!(r#"mkdir -p "{}""#, remote_path_str);
                    device.shell_command(&cmd, None, None);
                }
            }
        }
    }
}
