pub mod pull;
pub mod push;

use adb_client::{ADBDeviceExt, server::ADBServer, server_device::ADBServerDevice};
use clap::Parser;
use console::style;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{File, metadata};
use std::path::PathBuf;
use std::time::Duration;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;
const MIB_OVER_KIB: u64 = 1_024;
const GIB_OVER_KIB: u64 = 1_048_576;
const TIB_OVER_KIB: u64 = 1_073_741_824;
use crate::pull::pull_tools;
use crate::push::push_tools;
use std::path::Path;
use std::str::Lines;

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
        _ => panic!("Improper use of human_readable"),
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
    //    let cli_input = Cli::parse();

    let mut local_path = match env::home_dir() {
        Some(path) => path,
        None => panic!("No root path found"),
    };

    let config_file = File::open("config.json").expect("Cannot find config file");
    let config: ConfigFile = serde_json::from_reader(config_file).expect("Cannot read config file");

    let local_dir: String = config.local_dir;
    let remote_dir: String = config.remote_dir;
    let allow_hidden: bool = config.allow_hidden;

    local_path.push(local_dir);

    let mut server = ADBServer::default();
    let mut device = server.get_device().expect("Can't get device");

    let client_request: &str = "pull";

    if client_request == "push" {
        let (queue, add, change, total_file_size) =
            push_tools::fetch_changes(&local_path, &remote_dir, &mut device, allow_hidden);

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
        let shell_command: String = format!("df {} | tail -n 1", remote_dir);
        device
            .shell_command(&shell_command, Some(&mut stdout), None)
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
            push_tools::write_changes(
                queue,
                &local_path,
                &remote_dir,
                add,
                change,
                &mut device,
                allow_hidden,
            );
        }
    }

    if client_request == "pull" {
        let (queue, add, change, total_file_size) =
            pull_tools::fetch_changes(&local_path, &remote_dir, &mut device, allow_hidden);

        println!("Files to add: {}", add);
        println!("Files to change: {}", change);

        let update_file_size_str: &str = &format!("{}", total_file_size);
        let update_file_size_human_readable = human_readable(update_file_size_str);
        let update_file_size_filesize_type = filesize_type(update_file_size_str);
        println!(
            "Size of files to be changed: {:.2} {}",
            update_file_size_human_readable, update_file_size_filesize_type
        );
    }
}
