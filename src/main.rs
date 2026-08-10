use std::env;
use std::fs;
use std::process::Command;
use walkdir::WalkDir;

fn main() {
    let mut root_path = match env::home_dir() {
        Some(path) => path,
        None => panic!("No root path found"),
    };

    let local_dir: &str = "/Volumes/NETWORK-DRIVE/Music/Songs/";
    let remote_dir: &str = "storage/BF87-2316/Music/Songs/";

    root_path.push(local_dir);

    for entry in WalkDir::new(root_path) {
        match entry {
            Ok(path) => {
                let curr_path = path.path();

                if fs::metadata(curr_path).expect("Path not found").is_file() {
                    println!("Currently working on {:?}", curr_path);

                    let instance = Command::new("adb")
                        .arg("push --sync")
                        .arg(curr_path)
                        .arg(remote_dir)
                        .output();

                    match instance {
                        Err(_) => println!("Error at path {:?}", curr_path.display()),
                        Ok(_) => println!("Finished {:?}", curr_path.display()),
                    }
                }
            }
            // Avoid panic to traverse what we can.
            Err(e) => println!("{:?}", e),
        }
    }
}
