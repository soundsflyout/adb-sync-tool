pub mod pull_tools {

    use adb_client::{ADBDeviceExt, server_device::ADBServerDevice};
    use console::style;
    use indicatif::ProgressBar;
    use std::error::Error;
    use std::fs::metadata;
    use std::fs::write;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Command;
    use std::str::Lines;
    use std::time::Duration;
    use std::time::UNIX_EPOCH;

    pub struct Queue {
        pub queue: Vec<(PathBuf, bool)>,
        pub add: u64,
        pub change: u64,
        pub total_size: u64,
    }

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
        local_path: &Path,
        remote_dir: &String,
        device: &mut ADBServerDevice,
        allow_hidden: bool,
        ignore_changes: bool,
    ) -> Result<Queue, Box<dyn Error>> {
        let mut add: u64 = 0;
        let mut change: u64 = 0;
        let mut total_file_size: u64 = 0;

        let mut queue: Vec<(PathBuf, bool)> = Vec::new();
        // Walk directories recursively and record the file paths of files that need to be added/changed
        // as a vector.
        let directory_loader = ProgressBar::new_spinner();
        println!("Scanning directories...");
        directory_loader.enable_steady_tick(Duration::from_millis(100));

        let mut stdout = Vec::new();
        let shell_command: String = format!("find {} -type d", remote_dir);
        device.shell_command(&shell_command, Some(&mut stdout), None)?;
        let stdout_str: String = String::from_utf8(stdout)?;
        // Iterable walking through each directory recursively
        let directories: Lines = stdout_str.lines();
        for path in directories {
            queue.push((PathBuf::from(path), false));
        }

        //Do the same thing for files
        let mut stdout = Vec::new();
        let shell_command: String = format!("find {} -type f", remote_dir);
        device.shell_command(&shell_command, Some(&mut stdout), None)?;
        let stdout_str: String = String::from_utf8(stdout)?;
        let files: Lines = stdout_str.lines();

        let scan_length: u64 = stdout_str.bytes().filter(|&b| b == b'\n').count() as u64;

        directory_loader.finish();

        println!("Fetching changes...");
        let loading_bar = ProgressBar::new(scan_length);

        for entry in files {
            let remote_path = PathBuf::from(entry);
            let remote_path_str = remote_path.to_str().expect("Invalid path");

            let rel_path = remote_path.strip_prefix(remote_dir)?;

            //shadow the local_path input since it is not needed outside of here.
            let mut local_path = local_path.to_path_buf();
            local_path.push(rel_path);

            let modified_time: u64 = modified_time(&local_path);

            let is_hidden: bool = remote_path
                .file_name()
                .expect("Not appropriate file name")
                .to_string_lossy()
                .starts_with('.');

            if allow_hidden || !is_hidden {
                if modified_time == 0 {
                    add += 1;
                    total_file_size += device.stat(remote_path_str)?.file_size as u64;
                    queue.push((remote_path, true));
                } else if !ignore_changes
                    && modified_time < device.stat(remote_path_str)?.mod_time as u64
                {
                    change += 1;
                    total_file_size += device.stat(remote_path_str)?.file_size as u64;
                    queue.push((remote_path, true));
                }
            }

            loading_bar.inc(1);
        }
        loading_bar.finish();

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
    ) -> Result<(), Box<dyn Error>> {
        let total: u64 = queue.add + queue.change;
        let mut curr_idx: u64 = 1;

        for (path, is_file) in queue.queue {
            let mut curr_local_path = PathBuf::from(local_path);

            let rel_path = path.strip_prefix(remote_dir).expect("Error: wrong prefix");

            let rel_path_str = rel_path.to_str().expect("Not a string");

            curr_local_path.push(rel_path);
            let local_path_str = curr_local_path.to_str().expect("Not a path");

            let modified_time = modified_time(&curr_local_path) as u32;
            let remote_path_str = path.to_str().expect("This remote path is not valid");
            let remote_metadata = device.stat(remote_path_str)?;

            // Since this is unix time, a value of 0 means the file does not exist.
            if modified_time < remote_metadata.mod_time {
                if is_file {
                    if allow_hidden
                        || !&path
                            .file_name()
                            .expect("This remote path is not valid")
                            .to_string_lossy()
                            .starts_with('.')
                    {
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
                } else {
                    let cmd = format!(r#"mkdir -p "{}""#, local_path_str);
                    Command::new("sh").arg("-c").arg(cmd).output().unwrap();
                }
            }
        }
        Ok(())
    }
}
