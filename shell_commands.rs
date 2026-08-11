use std::process::{Command, Stdio};

fn main() {
    let child = Command::new("adb")
        .arg("push")
        .arg("--sync")
        .arg("/home/sparkle/Music/BeeMoved.flac")
        .arg("storage/self/primary/Music")
        .stdout(Stdio::piped())
        .spawn()
        .expect("err");

    let output = child.wait_with_output().expect("oops");
    let file_locs = String::from_utf8(output.stdout).expect("Uh oh");
    println!("{:?}", file_locs);
}
