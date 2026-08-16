pub mod config;
pub mod pull;
pub mod push;
pub mod queue;

const MIB_OVER_KIB: u64 = 1_024;
const GIB_OVER_KIB: u64 = 1_048_576;
const TIB_OVER_KIB: u64 = 1_073_741_824;
const CONFIG_PATH: &str = include_str!("../config.json");

use adb_client::{ADBDeviceExt, server::ADBServer};
use clap::Parser;
use dialoguer::Confirm;
use serde_json::value::Value;
use std::env;
use std::error::Error;
use std::fs::File;
use std::process::Command;
use std::process::Stdio;

use crate::config::ConfigFile;
use crate::pull::pull_tools;
use crate::push::push_tools;

#[derive(Parser)]
pub struct Cli {
    pub stream_dir: String, //push or pull
    pub alias: String,

    // Set --ignore_changes if you don't want to update changes
    #[arg(short, long, default_value_t = true)]
    pub ignore_changes: bool,
}

fn filesize_type(input: &str) -> String {
    let mut value: u64 = input.parse().expect("Not a valid number");
    if cfg!(target_os = "windows") {
        value /= 1024;
    }
    match value {
        0..MIB_OVER_KIB => String::from("KiB"),
        MIB_OVER_KIB..GIB_OVER_KIB => String::from("MiB"),
        GIB_OVER_KIB..TIB_OVER_KIB => String::from("GiB"),
        _ => String::from("TiB"),
    }
}

fn human_readable(input: &str) -> f64 {
    let filesize = filesize_type(input);
    let mut value: f64 = input.parse().expect("Not a valid number");
    if cfg!(target_os = "windows") {
        value /= 1024.0;
    }
    match filesize {
        x if x == "KiB" => value,
        x if x == "MiB" => value / (MIB_OVER_KIB as f64),
        x if x == "GiB" => value / (GIB_OVER_KIB as f64),
        x if x == "TiB" => value / (TIB_OVER_KIB as f64),
        _ => panic!("Improper use of human_readable"),
    }
}

fn check_enough_space(addition: u64, free_space: u64) -> bool {
    if free_space < 10024 + addition {
        println!("Not enough space on disk");
        false
    } else {
        Confirm::new()
            .with_prompt("Do you want to make changes?")
            .interact()
            .unwrap()
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli_input = Cli::parse();

    if !(cfg!(target_os = "macos") || cfg!(target_os = "linux")) {
        panic!("Unsupported OS. Only MacOS and Linux are currently supported.")
    }
    let mut config_path = match env::home_dir() {
        Some(path) => path,
        None => panic!("No root path found"),
    };
    config_path.push("adb-sync-tool/config.json");

    let config_file = match File::open(config_path) {
        Ok(file) => file,
        Err(_) => {
            println!(
                "Cannot find config file. Please read example_config.json or the readme for details. Exiting..."
            );
            return Ok(());
        }
    };

    let mut alias: Value = serde_json::from_reader(config_file).expect("Cannot read config file");
    let config: ConfigFile = serde_json::from_value(alias[cli_input.alias].take())?;

    let mut local_path = match env::home_dir() {
        Some(path) => path,
        None => panic!("No root path found"),
    };
    local_path.push(&config.local_dir);

    let mut server = ADBServer::default();
    let mut device = server.get_device().expect("Can't get device");

    if cli_input.stream_dir == "push" {
        let Ok(queue) =
            push_tools::fetch_changes(&config, &mut device, &local_path, cli_input.ignore_changes)
        else {
            panic!("Can't grab files. Do both the local and remote directories exist?")
        };

        println!("Files to add: {}", queue.add);
        println!("Files to change: {}", queue.change);

        if queue.add == 0 && queue.change == 0 {
            println!("No changes available. Exiting...");
            return Ok(());
        }

        let update_file_size_str: &str = &format!("{}", queue.total_size);
        let update_file_size_human_readable = human_readable(update_file_size_str);
        let update_file_size_filesize_type = filesize_type(update_file_size_str);
        println!(
            "Size of files to be changed: {:.2} {}",
            update_file_size_human_readable, update_file_size_filesize_type
        );

        let mut stdout = Vec::new();
        let shell_command: String = format!(r#"df "{}" | tail -n 1"#, config.remote_dir);
        device.shell_command(&shell_command, Some(&mut stdout), None)?;
        let stdout_str: String = String::from_utf8(stdout)?;
        let stdout_values: Vec<&str> = stdout_str.split_whitespace().collect();
        let free_human_readable = human_readable(stdout_values[3]);
        let free_filesize_type = filesize_type(stdout_values[3]);

        let free_dir_size: u64 = stdout_values[3].parse()?;

        println!(
            "Space available: {:.2} {}",
            free_human_readable, free_filesize_type
        );

        let confirmation: bool = check_enough_space(queue.total_size, free_dir_size);

        if !confirmation {
            println!("Exiting...");
        } else {
            push_tools::write_changes(queue, &config, &mut device, &local_path);
        }
    }

    if cli_input.stream_dir == "pull" {
        let Ok(queue) =
            pull_tools::fetch_changes(&config, &mut device, &local_path, cli_input.ignore_changes)
        else {
            panic!("Can't grab files. Do both the local and remote directories exist?")
        };

        println!("Files to add: {}", queue.add);
        println!("Files to change: {}", queue.change);

        if queue.add == 0 && queue.change == 0 {
            println!("No changes available. Exiting...");
            return Ok(());
        }

        let update_file_size_str: &str = &format!("{}", queue.total_size);
        let update_file_size_human_readable = human_readable(update_file_size_str);
        let update_file_size_filesize_type = filesize_type(update_file_size_str);
        println!(
            "Size of files to be changed: {:.2} {}",
            update_file_size_human_readable, update_file_size_filesize_type
        );
        let unix_command: String =
            format!(r#"df -k "{}" | tail -n 1"#, local_path.to_str().unwrap());

        let output = Command::new("sh")
            .arg("-c")
            .arg(&unix_command)
            .stdout(Stdio::piped())
            .output()?;

        let stdout_str = String::from_utf8(output.stdout).unwrap();
        let stdout_values: Vec<&str> = stdout_str.split_whitespace().collect();

        let free_space: &str = stdout_values[3];

        let free_human_readable = human_readable(free_space);
        let free_filesize_type = filesize_type(free_space);

        let free_dir_size: u64 = free_space.parse()?;

        println!(
            "Space available: {:.2} {}",
            free_human_readable, free_filesize_type
        );

        let confirmation: bool = check_enough_space(queue.total_size, free_dir_size);
        if !confirmation {
            println!("Exiting...");
        } else {
            pull_tools::write_changes(queue, &config, &mut device, &local_path)?
        }
    }
    Ok(())
}
