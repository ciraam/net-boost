use std::process::{Command, exit};

fn main() {
    let exe = std::env::current_exe().expect("current_exe");
    let script = exe.parent().expect("parent").join("windres.mjs");
    let status = Command::new("node")
        .arg(&script)
        .args(std::env::args_os().skip(1))
        .status()
        .expect("failed to invoke node windres.mjs");
    exit(status.code().unwrap_or(1));
}
