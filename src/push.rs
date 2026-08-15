pub mod push_tools {

    use adb_client::{ADBDeviceExt, server_device::ADBServerDevice};
    use console::style;
    use indicatif::ProgressBar;
    use std::error::Error;
    use std::fs::{File, metadata};
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::Duration;
    use std::time::UNIX_EPOCH;
    use walkdir::WalkDir;

    pub struct Queue {
        pub queue: Vec<PathBuf>,
        pub add: u64,
        pub change: u64,
        pub total_size: u64,
    }

    pub fn fetch_changes(
        local_path: &Path,
        remote_dir: &String,
        device: &mut ADBServerDevice,
        allow_hidden: bool,
    ) -> Result<Queue, Box<dyn Error>> {
        let mut add: u64 = 0;
        let mut change: u64 = 0;
        let mut total_file_size: u64 = 0;

        let abs_local_path = local_path.display().to_string();

        let mut queue: Vec<PathBuf> = Vec::new();
        // Walk directories recursively and record the file paths of files that need to be added/changed
        // as a vector.
        let directory_loader = ProgressBar::new_spinner();
        println!("Fetching changes...");
        directory_loader.enable_steady_tick(Duration::from_millis(100));

        for entry in WalkDir::new(local_path) {
            let path = entry?.path().to_path_buf();

            let rel_path = path
                .strip_prefix(&abs_local_path)
                .expect("Error: wrong prefix");

            let mut remote_path = PathBuf::from(remote_dir);
            remote_path.push(rel_path);
            let remote_path_str = remote_path
                .into_os_string()
                .into_string()
                .expect("Invalid path");

            let modified_time = device.stat(&remote_path_str)?.mod_time as u64;

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
                    .modified()?
                    .duration_since(UNIX_EPOCH)?
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

        Ok(Queue {
            queue,
            add,
            change,
            total_size: total_file_size,
        })
    }

    pub fn write_changes(
        queue: Queue,
        local_path: &Path,
        remote_dir: &String,
        device: &mut ADBServerDevice,
        allow_hidden: bool,
    ) {
        let total: u64 = queue.add + queue.change;
        let mut curr_idx: u64 = 1;

        let abs_local_path = local_path.display().to_string();
        for path in queue.queue {
            let path_metadata = metadata(&path).unwrap();
            let mut remote_path = PathBuf::from(remote_dir);

            let rel_path = path
                .strip_prefix(&abs_local_path)
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
                    device.shell_command(&cmd, None, None).unwrap();
                }
            }
        }
    }
}
