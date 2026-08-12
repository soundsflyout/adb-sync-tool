use adb_client::{ADBDeviceExt, server::ADBServer};
use std::env;
use std::fs::{File, metadata};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

fn main() {
    let mut root_path = match env::home_dir() {
        Some(path) => path,
        None => panic!("No root path found"),
    };

    let local_dir: &str = "Music/";
    let remote_dir: &str = "storage/BF87-2316/Music";

    root_path.push(local_dir);
    let abs_root_path = &root_path.display().to_string();

    let mut server = ADBServer::default();
    let mut device = server.get_device().expect("Can't get device");

    for entry in WalkDir::new(root_path) {
        match entry {
            Ok(path) => {
                let curr_path = path.path();

                println!("Currently working on {:?}", curr_path);
                let rel_path = curr_path
                    .strip_prefix(abs_root_path)
                    .expect("Error wrong prefix");

                let mut remote_path = PathBuf::from(remote_dir);
                remote_path.push(rel_path);
                let remote_path_str = remote_path
                    .into_os_string()
                    .into_string()
                    .expect("Invalid path");
                println!("Remote path is located at: {}", remote_path_str);

                let result = match device.stat(&remote_path_str) {
                    Ok(stats) => stats.mod_time as u64,
                    Err(_) => 0,
                };
                println!("Mod time: {}", result);

                let path_metadata = metadata(curr_path).expect("Path not found");

                // Since this is unix time, a value of 0 means the file does not exist.
                if result
                    < path_metadata
                        .modified()
                        .unwrap()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                {
                    if path_metadata.is_file() {
                        println!("Update needed");
                        let file_path = File::open(curr_path).expect("File not found!");
                        device
                            .push(file_path, remote_path_str)
                            .expect("File push error");
                    } else {
                        let cmd = format!(r#"mkdir -p "{}""#, remote_path_str);
                        device.shell_command(&cmd, None, None);
                    }
                }
            }
            // Avoid panic to traverse what we can.
            Err(e) => println!("{:?}", e),
        }
    }
}
