use bdd::cli::{validate_and_process, Cli};
use bdd::engine::run_pipeline;
use clap::Parser;

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    let cli = Cli::parse();

    if cli.mcp {
        if let Err(e) = bdd::mcp::run_mcp_server() {
            eprintln!("{}", e);
            std::process::exit(e.exit_code());
        }
        return;
    }

    if let Some(port) = cli.serve {
        if let Err(e) = bdd::server::run_server(port) {
            eprintln!("[bdd web] Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    if cli.list_presets {
        println!("{}", bdd::preset::format_presets_table());
        return;
    }

    if let Some(ref pat_opt) = cli.explain_pattern {
        let raw_pat = if !pat_opt.trim().is_empty() {
            pat_opt.as_str()
        } else if let Some(ref ip) = cli.input_pattern {
            ip.as_str()
        } else if let Some(ref pr) = cli.preset {
            if let Some(p) = bdd::preset::find_preset(pr) {
                p.pattern
            } else {
                eprintln!("Error: unknown preset '{}'", pr);
                std::process::exit(1);
            }
        } else {
            eprintln!(
                "Error: --explain-pattern requires a pattern string, --input-pattern, or --preset"
            );
            std::process::exit(1);
        };

        let pattern_to_explain = if let Some(p) = bdd::preset::find_preset(raw_pat) {
            p.pattern
        } else {
            raw_pat
        };

        match bdd::explain::explain_pattern(pattern_to_explain) {
            Ok(exp) => {
                if cli.output_json {
                    println!("{}", bdd::explain::format_explanation_json(&exp));
                } else {
                    println!("{}", bdd::explain::format_explanation_text(&exp));
                }
                return;
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(e.exit_code());
            }
        }
    }

    if let Some(ref probe_opt) = cli.probe {
        let target = if !probe_opt.trim().is_empty() {
            probe_opt.as_str()
        } else if cli.input_file != "-" {
            cli.input_file.as_str()
        } else {
            "-"
        };

        let report = if target == "-" {
            let mut stdin = std::io::stdin();
            bdd::probe::probe_reader(&mut stdin, "stdin")
        } else {
            match std::fs::File::open(target) {
                Ok(mut f) => bdd::probe::probe_reader(&mut f, target),
                Err(e) => {
                    eprintln!("Cannot open probe target '{}': {}", target, e);
                    std::process::exit(1);
                }
            }
        };

        match report {
            Ok(rep) => {
                if cli.output_json {
                    println!("{}", bdd::probe::format_probe_json(&rep));
                } else {
                    println!("{}", bdd::probe::format_probe_text(&rep));
                }
                return;
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(e.exit_code());
            }
        }
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
