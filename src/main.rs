mod agent;
mod cli;
mod config;
mod git;
mod graph;
mod init;
mod render;
mod runner;

fn main() {
    if let Err(error) = cli::run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
