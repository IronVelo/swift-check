use std::path::{Path, PathBuf};
use clap::{Command, Arg, Parser};

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    test: Option<String>,
    #[arg(short, long)]
    dir: Option<PathBuf>
}

fn main() {
    let cli = Cli::parse();
    println!("Hello, world!");
}
