//! Auxiliary CLI command handlers for bdd.
//!
//! Separates non-pipeline commands (schema introspection, completions, presets,
//! MCP launch, probe diagnostics) from the core streaming execution pipeline.

use crate::cli::Cli;
#[cfg(feature = "probe")]
use std::io::Read;

/// Dispatches auxiliary CLI commands if requested by flags.
/// Returns `Some(exit_code)` if a command was handled, or `None` if normal pipeline execution should proceed.
pub fn dispatch_auxiliary_commands(cli: &Cli) -> Option<i32> {
    if let Some(ref path) = cli.presets_file {
        crate::preset::set_custom_presets_path(path.clone());
    }

    if let Some(code) = handle_info_commands(cli) {
        return Some(code);
    }
    if let Some(code) = handle_preset_commands(cli) {
        return Some(code);
    }
    if let Some(code) = handle_explain_commands(cli) {
        return Some(code);
    }
    if let Some(code) = handle_probe_commands(cli) {
        return Some(code);
    }

    None
}

/// Handles `--llms`, `--completions`, and `--mcp`.
fn handle_info_commands(cli: &Cli) -> Option<i32> {
    if cli.llms {
        print!("{}", include_str!("../llms.txt"));
        return Some(0);
    }

    if let Some(shell) = cli.completions {
        use clap::CommandFactory;
        clap_complete::generate(shell, &mut Cli::command(), "bdd", &mut std::io::stdout());
        return Some(0);
    }

    if cli.mcp {
        let current_exe = std::env::current_exe().ok();
        let sibling_mcp = current_exe
            .as_ref()
            .and_then(|p| p.parent())
            .map(|d| d.join("bdd-mcp"));

        let status = if let Some(ref p) = sibling_mcp {
            if p.exists() {
                std::process::Command::new(p).status().ok()
            } else {
                std::process::Command::new("bdd-mcp").status().ok()
            }
        } else {
            std::process::Command::new("bdd-mcp").status().ok()
        };

        if let Some(s) = status {
            return Some(s.code().unwrap_or(0));
        } else {
            eprintln!("Notice: The MCP server has been decoupled to the standalone 'bdd-mcp' companion binary.");
            eprintln!("To launch the MCP server, run:");
            eprintln!("  bdd-mcp");
            eprintln!("or via cargo:");
            eprintln!("  cargo run -p bdd-mcp");
            return Some(1);
        }
    }

    None
}

/// Handles `--list-presets`.
fn handle_preset_commands(cli: &Cli) -> Option<i32> {
    if cli.list_presets {
        println!("{}", crate::preset::format_presets_table());
        return Some(0);
    }

    None
}

/// Handles `--explain-pattern`, `--export-c`, and `--export-rust`.
fn handle_explain_commands(cli: &Cli) -> Option<i32> {
    if let Some(ref pat_opt) = cli.explain_pattern {
        let raw_pat = if !pat_opt.trim().is_empty() {
            pat_opt.as_str()
        } else if let Some(ref ip) = cli.input_pattern {
            ip.as_str()
        } else if let Some(ref pr) = cli.preset {
            if let Some(p) = crate::preset::find_preset(pr) {
                p.pattern.as_str()
            } else {
                eprintln!("Error: unknown preset '{}'", pr);
                return Some(1);
            }
        } else {
            eprintln!(
                "Error: --explain-pattern requires a pattern string, --input-pattern, or --preset"
            );
            return Some(1);
        };

        let pattern_to_explain = if let Some(p) = crate::preset::find_preset(raw_pat) {
            p.pattern.as_str()
        } else {
            raw_pat
        };

        match crate::explain::explain_pattern(pattern_to_explain) {
            Ok(exp) => {
                if cli.output_json {
                    println!("{}", crate::explain::format_explanation_json(&exp));
                } else {
                    println!("{}", crate::explain::format_explanation_text(&exp));
                }
                return Some(0);
            }
            Err(e) => {
                eprintln!("{}", e);
                return Some(e.exit_code());
            }
        }
    }

    if cli.export_c || cli.export_rust {
        let raw_pat = if let Some(ref pat) = cli.input_pattern {
            pat.as_str()
        } else if let Some(ref pr) = cli.preset {
            if let Some(p) = crate::preset::find_preset(pr) {
                p.pattern.as_str()
            } else {
                eprintln!("Error: unknown preset '{}'", pr);
                return Some(1);
            }
        } else if let Some(ref exp) = cli.explain_pattern {
            if !exp.trim().is_empty() {
                exp.as_str()
            } else {
                eprintln!("Error: --export-c / --export-rust requires a pattern or preset");
                return Some(1);
            }
        } else if let Some(ref pat) = cli.stream_pattern {
            pat.as_str()
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
            return Some(1);
        };

        let pattern_to_export = if let Some(p) = crate::preset::find_preset(raw_pat) {
            p.pattern.as_str()
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
            crate::diag::warn(
                "'--export-c' is deprecated and will be removed in a future release. Use 'bdd --explain-pattern <PATTERN> --output-json | python3 contrib/python/json_to_c.py' instead."
            );
            match crate::explain::generate_c_struct(pattern_to_export, struct_name.as_deref()) {
                Ok(code) => {
                    print!("{}", code);
                    return Some(0);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    return Some(e.exit_code());
                }
            }
        } else if cli.export_rust {
            crate::diag::warn(
                "'--export-rust' is deprecated and will be removed in a future release. Use 'bdd --explain-pattern <PATTERN> --output-json | python3 contrib/python/json_to_rust.py' instead."
            );
            match crate::explain::generate_rust_struct(pattern_to_export, struct_name.as_deref()) {
                Ok(code) => {
                    print!("{}", code);
                    return Some(0);
                }
                Err(e) => {
                    eprintln!("{}", e);
                    return Some(e.exit_code());
                }
            }
        }
    }

    None
}

/// Handles `--probe` and `--probe-visual`.
#[cfg(feature = "probe")]
fn handle_probe_commands(cli: &Cli) -> Option<i32> {
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
            let buf = read_sample_prefix(std::io::stdin(), 1_048_576).unwrap_or_else(|e| {
                eprintln!("Error reading stdin: {}", e);
                std::process::exit(1);
            });
            (buf, "stdin".to_string())
        } else {
            match std::fs::File::open(target) {
                Ok(mut f) => {
                    #[cfg(feature = "mmap")]
                    let mmap_buf = if !cli.no_mmap {
                        f.metadata().ok().and_then(|meta| {
                            if meta.is_file() && meta.len() > 0 {
                                unsafe { memmap2::MmapOptions::new().map(&f).ok() }
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };

                    #[cfg(feature = "mmap")]
                    if let Some(mmap) = mmap_buf {
                        let sample_len = mmap.len().min(1_048_576);
                        (mmap[..sample_len].to_vec(), target.to_string())
                    } else {
                        let buf = read_sample_prefix(&mut f, 1_048_576).unwrap_or_else(|e| {
                            eprintln!("Cannot read probe target '{}': {}", target, e);
                            std::process::exit(1);
                        });
                        (buf, target.to_string())
                    }

                    #[cfg(not(feature = "mmap"))]
                    {
                        let buf = read_sample_prefix(&mut f, 1_048_576).unwrap_or_else(|e| {
                            eprintln!("Cannot read probe target '{}': {}", target, e);
                            std::process::exit(1);
                        });
                        (buf, target.to_string())
                    }
                }
                Err(e) => {
                    eprintln!("Cannot open probe target '{}': {}", target, e);
                    return Some(1);
                }
            }
        };

        let rep = crate::probe::probe_buffer(&buf, &target_name);
        if cli.output_json {
            println!("{}", crate::probe::format_probe_json(&rep));
        } else {
            println!("{}", crate::probe::format_probe_text(&rep));
            if cli.probe_visual {
                println!("\n{}", crate::probe::format_visual_entropy_map(&buf, 64));
            }
        }
        return Some(0);
    }

    None
}

#[cfg(not(feature = "probe"))]
fn handle_probe_commands(cli: &Cli) -> Option<i32> {
    if cli.probe.is_some() || cli.probe_visual {
        eprintln!(
            "Error: binary probing was requested, but bdd was compiled without the 'probe' feature. Recompile with '--features probe'."
        );
        return Some(1);
    }
    None
}

#[cfg(feature = "probe")]
fn read_sample_prefix<R: Read>(mut reader: R, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 65536];
    while buf.len() < limit {
        let to_read = (limit - buf.len()).min(chunk.len());
        match reader.read(&mut chunk[..to_read]) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(buf)
}
