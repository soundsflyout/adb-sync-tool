use adb_client::server::ADBServer;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs::{File, metadata};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

fn main() {
    let mut root_path = match env::home_dir() {
        Some(path) => path,
        None => panic!("No root path found"),
    };

    let local_dir: &str = "Music/";
    let remote_dir: &str = "storage/BF87-2316/Music";

    root_path.push(local_dir);
    let actual_path = &root_path.display().to_string();

    let mut server = ADBServer::default();
    let mut device = server.get_device().expect("Can't get device");
    let remote_files = &device.list(remote_dir).unwrap();
    device.shell_command("ls");

    for entry in WalkDir::new(root_path) {
        match entry {
            Ok(path) => {
                let curr_path = path.path();

                if metadata(curr_path).expect("Path not found").is_file() {
                    println!("Currently working on {:?}", curr_path);
                    curr_path.strip_prefix(actual_path);
                }
            }
            // Avoid panic to traverse what we can.
            Err(e) => println!("{:?}", e),
        }
    }
}
