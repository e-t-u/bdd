use bdd::cli::{validate_and_process, Cli};
use bdd::commands::dispatch_auxiliary_commands;
use bdd::engine::run_pipeline;
use clap::Parser;

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    let cli = Cli::parse();

    if let Some(exit_code) = dispatch_auxiliary_commands(&cli) {
        std::process::exit(exit_code);
    }

    let mut config = match validate_and_process(cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(e.exit_code());
        }
    };
    config.raw_args = raw_args;

    if let Err(e) = run_pipeline(config) {
        eprintln!("{}", e);
        std::process::exit(e.exit_code());
    }
}
