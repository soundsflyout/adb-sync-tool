use adb_client::{ADBDeviceExt, server::ADBServer};
use clap::Parser;
use console::style;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{File, metadata};
use std::path::PathBuf;
use std::time::Duration;
use walkdir::WalkDir;
const MIB_OVER_KIB: u64 = 1_024;
const GIB_OVER_KIB: u64 = 1_048_576;
const TIB_OVER_KIB: u64 = 1_073_741_824;

#[derive(Parser)]
struct Cli {
    stream_dir: String, //push or pull
    alias: String,
}

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    local_dir: String,
    remote_dir: String,
    allow_hidden: bool,
}

fn filesize_type(input: &str) -> String {
    let value: u64 = input.parse().expect("Not a valid number");
    match value {
        0..MIB_OVER_KIB => String::from("KiB"),
        MIB_OVER_KIB..GIB_OVER_KIB => String::from("MiB"),
        GIB_OVER_KIB..TIB_OVER_KIB => String::from("GiB"),
        _ => String::from("TiB"),
    }
}

fn human_readable(input: &str) -> f64 {
    let filesize = filesize_type(input);
    let value: f64 = input.parse().expect("Not a valid number");
    match filesize {
        x if x == "KiB" => value,
        x if x == "MiB" => value / (MIB_OVER_KIB as f64),
        x if x == "GiB" => value / (GIB_OVER_KIB as f64),
        x if x == "TiB" => value / (TIB_OVER_KIB as f64),
        _ => 0.0,
    }
}

fn check_enough_space(addition: u64, total_size: u64, free_space: u64) -> bool {
    if free_space - addition < 10024 {
        println!("Not enough space on disk");
        false
    } else if ((free_space - addition) as f64) < (total_size as f64) * 0.05 {
        Confirm::new()
            .with_prompt("Warning: less than 5% of disk space after change. Continue?")
            .interact()
            .unwrap()
    } else {
        Confirm::new()
            .with_prompt("Do you want to make changes?")
            .interact()
            .unwrap()
    }
}

fn main() {
    let cli_input = Cli::parse();

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
    let mut total_file_size: u64 = 0;

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
                    total_file_size += path_metadata.len();
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
                total_file_size += path_metadata.len();
            }
            queue.push(path);
        }
    }
    directory_loader.finish();

    // Convert file in bytes to corresponding kiB
    total_file_size /= 1024;

    println!("Files to add: {}", add);
    println!("Files to change: {}", change);

    let update_file_size_str: &str = &format!("{}", total_file_size);
    let update_file_size_human_readable = human_readable(update_file_size_str);
    let update_file_size_filesize_type = filesize_type(update_file_size_str);
    println!(
        "Size of files to be changed: {:.2} {}",
        update_file_size_human_readable, update_file_size_filesize_type
    );

    let mut stdout = Vec::new();
    device
        .shell_command(
            &"df /storage/BF87-2316 | tail -n 1",
            Some(&mut stdout),
            None,
        )
        .unwrap();
    let stdout_str: String = String::from_utf8(stdout).unwrap();
    let stdout_values: Vec<&str> = stdout_str.split_whitespace().collect();
    let free_human_readable = human_readable(stdout_values[3]);
    let free_filesize_type = filesize_type(stdout_values[3]);

    let total_dir_size: u64 = stdout_values[1].parse().expect("Not a valid number");
    let free_dir_size: u64 = stdout_values[3].parse().expect("Not a valid number");

    println!(
        "Space available: {:.2} {}",
        free_human_readable, free_filesize_type
    );

    let confirmation: bool = check_enough_space(total_file_size, total_dir_size, free_dir_size);

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
