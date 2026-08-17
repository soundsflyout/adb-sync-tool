pub mod push_tools {
    use adb_client::{ADBDeviceExt, server_device::ADBServerDevice};
    use console::style;
    use indicatif::ProgressBar;
    use std::error::Error;
    use std::fs::{File, metadata};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::str::Lines;
    use std::time::Duration;
    use std::time::UNIX_EPOCH;

    use crate::config::ConfigFile;
    use crate::queue::Queue;

    pub fn fetch_changes(
        config: &ConfigFile,
        device: &mut ADBServerDevice,
        local_path: &Path,
        ignore_changes: bool,
    ) -> Result<Queue, Box<dyn Error>> {
        let mut add: u64 = 0;
        let mut change: u64 = 0;
        let mut total_file_size: u64 = 0;

        let abs_local_path = local_path.display().to_string();

        let mut dir_queue: Vec<PathBuf> = Vec::new();
        let mut file_queue: Vec<PathBuf> = Vec::new();
        let directory_loader = ProgressBar::new_spinner();
        println!("Scanning directories...");
        directory_loader.enable_steady_tick(Duration::from_millis(100));

        let shell_command: String = format!("find {} -type d", abs_local_path);
        let output = Command::new("sh").arg("-c").arg(&shell_command).output()?;
        let stdout_str = String::from_utf8(output.stdout)?;

        // Iterable walking through each directory recursively
        let directories: Lines = stdout_str.lines();
        // Add directories to queue
        for path in directories {
            let path_buf = PathBuf::from(path);
            if config.allow_hidden
                || !path_buf
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with('.')
            {
                dir_queue.push(path_buf);
            }
        }

        //Do the same thing for files
        let shell_command: String = format!("find {} -type f", abs_local_path);
        let output = Command::new("sh").arg("-c").arg(&shell_command).output()?;
        let stdout_str = String::from_utf8(output.stdout)?;
        let files: Lines = stdout_str.lines();

        let scan_length: u64 = stdout_str.bytes().filter(|&b| b == b'\n').count() as u64;

        directory_loader.finish();

        println!("Fetching changes...");
        let loading_bar = ProgressBar::new(scan_length);

        for entry in files {
            let local_path = PathBuf::from(entry);

            let rel_path = local_path.strip_prefix(&abs_local_path)?;

            let mut remote_path = PathBuf::from(&config.remote_dir);
            remote_path.push(rel_path);
            let remote_path_str = remote_path.to_str().expect("Invalid path");

            let modified_time = device.stat(remote_path_str)?.mod_time as u64;

            let path_metadata = metadata(&local_path)?;
            let is_hidden = local_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with('.');

            if config.allow_hidden || !is_hidden {
                if modified_time == 0 {
                    add += 1;
                    total_file_size += path_metadata.len();
                    file_queue.push(local_path);
                } else if !ignore_changes
                    && modified_time < device.stat(remote_path_str)?.mod_time as u64
                {
                    change += 1;
                    total_file_size += path_metadata.len();
                    file_queue.push(local_path);
                }
            }

            loading_bar.inc(1);
        }
        loading_bar.finish();

        // Convert file in bytes to corresponding kiB
        total_file_size /= 1024;

        Ok(Queue {
            dir_queue,
            file_queue,
            add,
            change,
            total_size: total_file_size,
        })
    }

    pub fn write_changes(
        queue: Queue,
        config: &ConfigFile,
        device: &mut ADBServerDevice,
        local_path: &Path,
    ) -> Result<(), Box<dyn Error>> {
        let total: u64 = queue.add + queue.change;
        let mut curr_idx: u64 = 1;

        let abs_local_path = local_path.to_str().unwrap();

        let directory_loader = ProgressBar::new_spinner();
        println!("Initializing directories...");
        directory_loader.enable_steady_tick(Duration::from_millis(100));
        for path in queue.dir_queue {
            let mut remote_path = PathBuf::from(&config.remote_dir);

            let rel_path = path.strip_prefix(abs_local_path)?;

            remote_path.push(rel_path);
            let remote_path_str = remote_path.to_str().expect("Invalid path");
            let cmd = format!(r#"mkdir -p "{}""#, remote_path_str);
            device.shell_command(&cmd, None, None)?;
        }
        directory_loader.finish();

        for path in queue.file_queue {
            let path_metadata = metadata(&path)?;
            let mut remote_path = PathBuf::from(&config.remote_dir);

            let rel_path = path
                .strip_prefix(abs_local_path)
                .expect("Error: wrong prefix");

            let rel_path_str = rel_path.to_str().unwrap();

            remote_path.push(rel_path);
            let remote_path_str = remote_path.to_str().expect("Invalid path");

            let modified_time = device.stat(remote_path_str)?.mod_time as u64;
            // Since this is unix time, a value of 0 means the file does not exist.
            if modified_time
                < path_metadata
                    .modified()?
                    .duration_since(UNIX_EPOCH)?
                    .as_secs()
            {
                let file_path = File::open(&path).expect("File not found!");
                if config.allow_hidden
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
                    device.push(file_path, remote_path_str)?;
                    curr_idx += 1;
                }
            }
        }
        Ok(())
    }
}
