pub mod pull_tools {

    use adb_client::{ADBDeviceExt, server_device::ADBServerDevice};
    use console::style;
    use indicatif::ProgressBar;
    use std::fs::{File, metadata};
    use std::path::Path;
    use std::path::PathBuf;
    use std::str::Lines;
    use std::time::Duration;
    use std::time::UNIX_EPOCH;

    pub fn fetch_changes(
        local_path: &Path,
        remote_dir: &String,
        device: &mut ADBServerDevice,
        allow_hidden: bool,
    ) -> (Vec<PathBuf>, u64, u64, u64) {
        let mut add: u64 = 0;
        let mut change: u64 = 0;
        let mut total_file_size: u64 = 0;

        let mut queue: Vec<PathBuf> = Vec::new();
        // Walk directories recursively and record the file paths of files that need to be added/changed
        // as a vector.
        let directory_loader = ProgressBar::new_spinner();
        println!("Fetching changes...");
        directory_loader.enable_steady_tick(Duration::from_millis(100));

        let mut stdout = Vec::new();
        let shell_command: String = format!("find {}", remote_dir);
        device
            .shell_command(&shell_command, Some(&mut stdout), None)
            .unwrap();
        let stdout_str: String = String::from_utf8(stdout).unwrap();
        // Iterable walking through each directory recursively
        let stdout_values: Lines = stdout_str.lines();

        for entry in stdout_values {
            let remote_path = PathBuf::from(entry);
            let remote_path_str = remote_path
                .clone()
                .into_os_string()
                .into_string()
                .expect("Invalid path");

            let rel_path = remote_path
                .strip_prefix(remote_dir)
                .expect("Error: wrong prefix");

            //shadow the local_path input since it is not needed outside of here.
            let mut local_path = local_path.to_path_buf();
            local_path.push(rel_path);

            let modified_time: u64 = match metadata(&local_path) {
                Ok(path_metadata) => path_metadata
                    .modified()
                    .unwrap()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                Err(_) => 0,
            };

            //let mut stdout = Vec::new();
            //let shell_command = format!(r#"test -f "{}" && echo 0 || echo 1"#, remote_path_str);
            //device
            //    .shell_command(&shell_command, Some(&mut stdout), None)
            //    .unwrap();
            //let stdout_str: String = String::from_utf8(stdout).unwrap();
            //let is_file = stdout_str.starts_with('0');

            let is_file = device.list(&remote_path_str).unwrap().is_empty();

            if modified_time == 0 {
                if allow_hidden
                    || !&remote_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with('.')
                {
                    if is_file {
                        add += 1;
                        //    total_file_size += path_metadata.len();
                    }
                    queue.push(remote_path);
                }
            } else if (modified_time < (device.stat(&remote_path_str).unwrap().mod_time as u64))
                && (allow_hidden
                    || !&remote_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with('.'))
            {
                if is_file {
                    change += 1;
                    //   total_file_size += path_metadata.len();
                }
                queue.push(remote_path);
            }
        }
        directory_loader.finish();

        // Convert file in bytes to corresponding kiB
        total_file_size /= 1024;

        (queue, add, change, total_file_size)
    }
}
