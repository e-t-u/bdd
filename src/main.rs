use bdd::cli::{validate_and_process, Cli};
use bdd::engine::run_pipeline;
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    let config = match validate_and_process(cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(e.exit_code());
        }
    };

    if let Err(e) = run_pipeline(config) {
        eprintln!("{}", e);
        std::process::exit(e.exit_code());
    }
}
