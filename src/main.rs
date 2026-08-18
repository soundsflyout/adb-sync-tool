pub mod config;
pub mod pull;
pub mod push;
pub mod queue;

const MIB_OVER_KIB: i64 = 1_024;
const GIB_OVER_KIB: i64 = 1_048_576;
const TIB_OVER_KIB: i64 = 1_073_741_824;
// Sets a buffer to leave 10 MiB of space left
// on device after push/pull.
const FILE_SPACE_BUFFER: i64 = 10_024;

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
struct Cli {
    stream_dir: String, //push or pull
    alias: Option<String>,

    // Set --ignore_changes if you don't want to update changes
    #[arg(short, long, default_value_t = false)]
    ignore_changes: bool,

    // Set --delete if you want to allow the program to delete
    // files in target not found in source.
    #[arg(short, long, default_value_t = false)]
    delete: bool,
}

enum FilesizeType {
    Kibibyte,
    Mebibyte,
    Gibibyte,
    Tebibyte,
}

impl FilesizeType {
    fn display(&self) -> String {
        match self {
            FilesizeType::Kibibyte => String::from("KiB"),
            FilesizeType::Mebibyte => String::from("MiB"),
            FilesizeType::Gibibyte => String::from("GiB"),
            FilesizeType::Tebibyte => String::from("TiB"),
        }
    }
}

fn filesize_type(input: &str) -> FilesizeType {
    let value: i64 = input.parse().expect("Not a valid number");
    match value {
        0..MIB_OVER_KIB => FilesizeType::Kibibyte,
        MIB_OVER_KIB..GIB_OVER_KIB => FilesizeType::Mebibyte,
        GIB_OVER_KIB..TIB_OVER_KIB => FilesizeType::Gibibyte,
        _ => FilesizeType::Tebibyte,
    }
}

fn human_readable(input: &str) -> f64 {
    let filesize = filesize_type(input);
    let value: f64 = input.parse().expect("Not a valid number");
    match filesize {
        FilesizeType::Kibibyte => value,
        FilesizeType::Mebibyte => value / (MIB_OVER_KIB as f64),
        FilesizeType::Gibibyte => value / (GIB_OVER_KIB as f64),
        FilesizeType::Tebibyte => value / (TIB_OVER_KIB as f64),
    }
}

fn check_enough_space(addition: i64, free_space: i64) -> bool {
    if free_space < FILE_SPACE_BUFFER + addition {
        println!("Not enough space on disk");
        return false;
    }
    Confirm::new()
        .with_prompt("Do you want to make changes?")
        .interact()
        .expect("Unsupported input")
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli_input = Cli::parse();

    if !(cfg!(target_os = "macos") || cfg!(target_os = "linux")) {
        panic!("Unsupported OS. Only MacOS and Linux are currently supported.")
    }

    let mut server = ADBServer::default();
    let mut device = server.get_device().expect("Can't get device");

    if cli_input.stream_dir == "storage" {
        let mut stdout = Vec::new();
        device.shell_command(&"df -h", Some(&mut stdout), None)?;
        let stdout_str: String = String::from_utf8(stdout)?;
        println!("{}", stdout_str);
        return Ok(());
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

    let Some(cli_alias) = cli_input.alias else {
        panic!("Error: Missing alias")
    };

    let mut alias: Value = serde_json::from_reader(config_file).expect("Cannot read config file");
    let config: ConfigFile = serde_json::from_value(alias[cli_alias].take())?;

    let mut local_path = match env::home_dir() {
        Some(path) => path,
        None => panic!("No root path found"),
    };
    local_path.push(&config.local_dir);

    if cli_input.stream_dir == "push" {
        let Ok(queue) = push_tools::fetch_changes(
            &config,
            &mut device,
            &local_path,
            cli_input.ignore_changes,
            cli_input.delete,
        ) else {
            panic!("Can't grab files. Do both the local and remote directories exist?")
        };

        println!("Files to add: {}", queue.add);
        println!("Files to change: {}", queue.change);
        println!("Files to delete {}", queue.del);

        if queue.add == 0 && queue.change == 0 && (!cli_input.delete || queue.del == 0) {
            println!("No changes available. Exiting...");
            return Ok(());
        }

        let is_neg: bool = queue.total_size < 0;
        let update_file_size_str: &str = if is_neg {
            &format!("{}", -queue.total_size)
        } else {
            &format!("{}", queue.total_size)
        };
        let update_file_size_human_readable = human_readable(update_file_size_str);
        let update_file_size_filesize_type = filesize_type(update_file_size_str).display();
        if is_neg {
            println!(
                "Size of files to be changed: -{:.2} {}",
                update_file_size_human_readable, update_file_size_filesize_type
            );
        } else {
            println!(
                "Size of files to be changed: {:.2} {}",
                update_file_size_human_readable, update_file_size_filesize_type
            );
        }

        let mut stdout = Vec::new();
        let shell_command: String = format!(r#"df "{}" | tail -n 1"#, config.remote_dir);
        device.shell_command(&shell_command, Some(&mut stdout), None)?;
        let stdout_str: String = String::from_utf8(stdout)?;
        let stdout_values: Vec<&str> = stdout_str.split_whitespace().collect();
        let free_human_readable = human_readable(stdout_values[3]);
        let free_filesize_type = filesize_type(stdout_values[3]).display();

        let free_dir_size: i64 = stdout_values[3].parse()?;

        println!(
            "Space available: {:.2} {}",
            free_human_readable, free_filesize_type
        );

        let confirmation: bool = check_enough_space(queue.total_size, free_dir_size);

        if !confirmation {
            println!("Exiting...");
        } else {
            push_tools::write_changes(queue, &config, &mut device, &local_path, cli_input.delete)?;
        }
    }

    if cli_input.stream_dir == "pull" {
        let Ok(queue) = pull_tools::fetch_changes(
            &config,
            &mut device,
            &local_path,
            cli_input.ignore_changes,
            cli_input.delete,
        ) else {
            panic!("Can't grab files. Do both the local and remote directories exist?")
        };

        println!("Files to add: {}", queue.add);
        println!("Files to change: {}", queue.change);
        println!("Files to delete {}", queue.del);

        if queue.add == 0 && queue.change == 0 && (!cli_input.delete || queue.del == 0) {
            println!("No changes available. Exiting...");
            return Ok(());
        }

        let is_neg: bool = queue.total_size < 0;
        let update_file_size_str: &str = if is_neg {
            &format!("{}", -queue.total_size)
        } else {
            &format!("{}", queue.total_size)
        };
        let update_file_size_human_readable = human_readable(update_file_size_str);
        let update_file_size_filesize_type = filesize_type(update_file_size_str).display();
        if is_neg {
            println!(
                "Size of files to be changed: -{:.2} {}",
                update_file_size_human_readable, update_file_size_filesize_type
            );
        } else {
            println!(
                "Size of files to be changed: {:.2} {}",
                update_file_size_human_readable, update_file_size_filesize_type
            );
        }
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
        let free_filesize_type = filesize_type(free_space).display();

        let free_dir_size: i64 = free_space.parse()?;

        println!(
            "Space available: {:.2} {}",
            free_human_readable, free_filesize_type
        );

        let confirmation: bool = check_enough_space(queue.total_size, free_dir_size);
        if !confirmation {
            println!("Exiting...");
        } else {
            pull_tools::write_changes(queue, &config, &mut device, &local_path, cli_input.delete)?
        }
    }
    Ok(())
}
