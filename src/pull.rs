pub mod pull_tools {
    use adb_client::{ADBDeviceExt, server_device::ADBServerDevice};
    use console::style;
    use indicatif::ProgressBar;
    use std::error::Error;
    use std::fs::metadata;
    use std::fs::write;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::str::Lines;
    use std::time::Duration;
    use std::time::UNIX_EPOCH;

    use crate::config::ConfigFile;
    use crate::queue::Queue;

    fn modified_time(local_path: &Path) -> u64 {
        match metadata(local_path) {
            Ok(path_metadata) => path_metadata
                .modified()
                .unwrap()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Err(_) => 0,
        }
    }

    pub fn fetch_changes(
        config: &ConfigFile,
        device: &mut ADBServerDevice,
        local_path: &Path,
        ignore_changes: bool,
    ) -> Result<Queue, Box<dyn Error>> {
        let mut add: u64 = 0;
        let mut change: u64 = 0;
        let mut total_file_size: i64 = 0;

        let mut dir_queue: Vec<PathBuf> = Vec::new();
        let mut file_queue: Vec<PathBuf> = Vec::new();

        let mut stdout = Vec::new();
        let shell_command: String = format!("find {} -type f", config.remote_dir);
        device.shell_command(&shell_command, Some(&mut stdout), None)?;
        let stdout_str: String = String::from_utf8(stdout)?;
        let files: Lines = stdout_str.lines();

        let scan_length: u64 = stdout_str.bytes().filter(|&b| b == b'\n').count() as u64;

        println!("Fetching changes...");
        let loading_bar = ProgressBar::new(scan_length);

        for entry in files {
            let remote_path = PathBuf::from(entry);
            let remote_path_str = remote_path.to_str().expect("Invalid path");

            let rel_path = remote_path.strip_prefix(&config.remote_dir)?;

            //shadow the local_path input since it is not needed outside of here.
            let mut local_path = local_path.to_path_buf();
            local_path.push(rel_path);
            let modified_time: u64 = modified_time(&local_path);
            let local_file_size: i64 = match metadata(&local_path) {
                Ok(metadata) => metadata.len() as i64,
                Err(_) => 0,
            };

            let remote_mod_time = device.stat(remote_path_str)?.mod_time as u64;

            //checks if the file or any of its parent directories are hidden.
            let is_hidden: bool = remote_path.to_str().unwrap().contains("/.");

            if (config.allow_hidden || !is_hidden)
                && (modified_time == 0 || (!ignore_changes && modified_time < remote_mod_time))
            {
                let parent_dir = local_path.parent().expect("Cannot find parent directory");
                let path_buf = PathBuf::from(parent_dir);
                let is_added: bool = match dir_queue.last() {
                    Some(x) => x == parent_dir,
                    None => false,
                };
                if !is_added {
                    dir_queue.push(path_buf);
                }
                if modified_time == 0 {
                    add += 1;
                    total_file_size += device.stat(remote_path_str)?.file_size as i64;
                } else {
                    change += 1;
                    total_file_size +=
                        device.stat(remote_path_str)?.file_size as i64 - local_file_size;
                }
                file_queue.push(remote_path);
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

        let directory_loader = ProgressBar::new_spinner();
        println!("Initializing directories...");
        directory_loader.enable_steady_tick(Duration::from_millis(100));
        for path in queue.dir_queue {
            let local_path_str = path.to_str().expect("Not a path");
            let cmd = format!(r#"mkdir -p "{}""#, local_path_str);
            Command::new("sh").arg("-c").arg(cmd).output()?;
        }
        directory_loader.finish();

        for path in queue.file_queue {
            let mut curr_local_path = PathBuf::from(&local_path);

            let rel_path = path
                .strip_prefix(&config.remote_dir)
                .expect("Error: wrong prefix");

            let rel_path_str = rel_path.to_str().expect("Not a string");

            curr_local_path.push(rel_path);
            let local_path_str = curr_local_path.to_str().expect("Not a path");

            let modified_time = modified_time(&curr_local_path) as u32;
            let remote_path_str = path.to_str().expect("This remote path is not valid");
            let remote_mod_time = device.stat(remote_path_str)?.mod_time;

            // Since this is unix time, a value of 0 means the file does not exist.
            if modified_time < remote_mod_time {
                let add_or_update: &str = if modified_time == 0 {
                    "Adding"
                } else {
                    "Updating"
                };
                let curr_idx_str = format!("[{}/{}]", curr_idx, total);
                let push_message = format!("{} {}", add_or_update, rel_path_str);
                println!("{} {}", style(curr_idx_str).bold().dim(), push_message);
                let mut stdout = Vec::new();
                device.pull(&String::from(remote_path_str), &mut stdout)?;
                write(local_path_str, stdout)?;
                curr_idx += 1;
            }
        }
        Ok(())
    }
}
