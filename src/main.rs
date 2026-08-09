use clap::Parser;

#[derive(Parser)]
struct Cli {
    command: String,
    local: std::path::PathBuf,
    remote: std::path::PathBuf,
}

fn main() {
    let args = Cli::parse();
    let local_path = args.local
}
