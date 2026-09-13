use bdd::cli::{validate_and_process, Cli};
use bdd::engine::run_pipeline;
use clap::Parser;

fn main() {
    let raw_args: Vec<String> = std::env::args().collect();
    let cli = Cli::parse();

    if cli.llms {
        print!("{}", include_str!("../llms.txt"));
        return;
    }

    if let Some(shell) = cli.completions {
        use clap::CommandFactory;
        clap_complete::generate(shell, &mut Cli::command(), "bdd", &mut std::io::stdout());
        return;
    }

    if cli.mcp {
        if let Err(e) = bdd::mcp::run_mcp_server() {
            eprintln!("{}", e);
            std::process::exit(e.exit_code());
        }
        return;
    }

    #[cfg(feature = "server")]
    if let Some(port) = cli.serve {
        if let Err(e) = bdd::server::run_server(port) {
            eprintln!("[bdd web] Error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    #[cfg(not(feature = "server"))]
    if cli.serve.is_some() {
        eprintln!("Error: The --serve web UI feature was not enabled at compile time. Recompile with --features server.");
        std::process::exit(1);
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

    if cli.export_c || cli.export_rust {
        let raw_pat = if let Some(ref pat) = cli.input_pattern {
            pat.as_str()
        } else if let Some(ref pr) = cli.preset {
            if let Some(p) = bdd::preset::find_preset(pr) {
                p.pattern
            } else {
                eprintln!("Error: unknown preset '{}'", pr);
                std::process::exit(1);
            }
        } else if let Some(ref exp) = cli.explain_pattern {
            if !exp.trim().is_empty() {
                exp.as_str()
            } else {
                eprintln!("Error: --export-c / --export-rust requires a pattern or preset");
                std::process::exit(1);
            }
        } else if cli.input_file != "-"
            && !cli.input_file.is_empty()
            && (cli.input_file.contains(':')
                || cli.input_file.contains('*')
                || cli.input_file.contains(','))
        {
            cli.input_file.as_str()
        } else {
            eprintln!(
                "Error: --export-c / --export-rust requires --input-pattern, --preset, or pattern positional"
            );
            std::process::exit(1);
        };

        let pattern_to_export = if let Some(p) = bdd::preset::find_preset(raw_pat) {
            p.pattern
        } else {
            raw_pat
        };

        let struct_name = cli.preset.as_deref().map(|p| {
            let mut name = String::new();
            for part in p.split(['-', '_']) {
                let mut chars = part.chars();
                if let Some(first) = chars.next() {
                    name.push(first.to_ascii_uppercase());
                    name.extend(chars.map(|c| c.to_ascii_lowercase()));
                }
            }
            name
        });

        if cli.export_c {
            match bdd::explain::generate_c_struct(pattern_to_export, struct_name.as_deref()) {
                Ok(code) => print!("{}", code),
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(e.exit_code());
                }
            }
        } else if cli.export_rust {
            match bdd::explain::generate_rust_struct(pattern_to_export, struct_name.as_deref()) {
                Ok(code) => print!("{}", code),
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(e.exit_code());
                }
            }
        }
        return;
    }

    if cli.probe.is_some() || cli.probe_visual {
        let probe_opt = cli.probe.as_deref().unwrap_or("");
        let target = if !probe_opt.trim().is_empty() {
            probe_opt
        } else if cli.input_file != "-" {
            cli.input_file.as_str()
        } else {
            "-"
        };

        let (buf, target_name) = if target == "-" {
            let mut stdin = std::io::stdin();
            let mut b = Vec::new();
            let mut chunk = [0u8; 65536];
            while b.len() < 1_048_576 {
                let to_read = (1_048_576 - b.len()).min(chunk.len());
                match std::io::Read::read(&mut stdin, &mut chunk[..to_read]) {
                    Ok(0) => break,
                    Ok(n) => b.extend_from_slice(&chunk[..n]),
                    Err(e) => {
                        eprintln!("Error reading stdin: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            (b, "stdin".to_string())
        } else {
            match std::fs::File::open(target) {
                Ok(mut f) => {
                    let mut b = Vec::new();
                    let mut chunk = [0u8; 65536];
                    while b.len() < 1_048_576 {
                        let to_read = (1_048_576 - b.len()).min(chunk.len());
                        match std::io::Read::read(&mut f, &mut chunk[..to_read]) {
                            Ok(0) => break,
                            Ok(n) => b.extend_from_slice(&chunk[..n]),
                            Err(e) => {
                                eprintln!("Cannot read probe target '{}': {}", target, e);
                                std::process::exit(1);
                            }
                        }
                    }
                    (b, target.to_string())
                }
                Err(e) => {
                    eprintln!("Cannot open probe target '{}': {}", target, e);
                    std::process::exit(1);
                }
            }
        };

        let rep = bdd::probe::probe_buffer(&buf, &target_name);
        if cli.output_json {
            println!("{}", bdd::probe::format_probe_json(&rep));
        } else {
            println!("{}", bdd::probe::format_probe_text(&rep));
            if cli.probe_visual {
                println!("\n{}", bdd::probe::format_visual_entropy_map(&buf, 64));
            }
        }
        return;
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
