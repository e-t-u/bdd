//! Model Context Protocol (MCP) server for bdd.
//!
//! Provides a standard JSON-RPC 2.0 stdio server interface enabling AI models
//! and coding assistants (Antigravity, Claude, Cursor) to slice, unpack, inspect,
//! probe, transcode, and generate binary streams natively via tool calls.

use bdd::cli::{validate_and_process, Cli};
use bdd::error::BddError;
use bdd::explain::{explain_pattern, format_explanation_json};
use bdd::preset::all_presets;
use bdd::probe::{format_probe_json, probe_buffer};
use clap::Parser;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{BufRead, Read, Write};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

fn run_mcp_server() -> Result<(), BddError> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = stdin.lock();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = request.get("id").cloned();

        match method {
            "initialize" => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "bdd-mcp",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                });
                writeln!(stdout, "{}", serde_json::to_string(&response).unwrap()).unwrap();
                stdout.flush().unwrap();
            }
            "notifications/initialized" => {
                // Client acknowledgment; no response required
            }
            "ping" => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {}
                });
                writeln!(stdout, "{}", serde_json::to_string(&response).unwrap()).unwrap();
                stdout.flush().unwrap();
            }
            "tools/list" => {
                let tools = json!([
                    {
                        "name": "bdd_slice",
                        "description": "Slice, unpack, extract, or convert unaligned bitstream fields from a file into structured JSON records.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string", "description": "Absolute path to binary file" },
                                "pattern": { "type": "string", "description": "Bit pattern, e.g. 'sync:11u,version:2u' or '4*8B'" },
                                "preset": { "type": "string", "description": "Optional preset name (e.g. 'mp3-header', 'mpeg-ts', 'nvfp4', 'wav-header')" },
                                "count": { "type": "integer", "description": "Maximum number of units/tuples to extract" },
                                "skip_units": { "type": "integer", "description": "Number of units to skip" },
                                "skip_bits": { "type": "integer", "description": "Initial bits to skip" },
                                "raw_unit": { "type": "integer", "description": "Raw container unit size in bits" },
                                "offset": { "type": "integer", "description": "Offset within raw container unit in bits" },
                                "json_fields": { "type": "string", "description": "Comma-separated custom JSON field names" }
                            }
                        }
                    },
                    {
                        "name": "bdd_probe",
                        "description": "Analyze binary file characteristics, Shannon entropy, byte distributions, and repeating record strides.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string", "description": "Absolute path to binary file" }
                            },
                            "required": ["file_path"]
                        }
                    },
                    {
                        "name": "bdd_probe_units",
                        "description": "Probe unit stream characteristics and entropy AFTER input stream processing, including detection of maximum-entropy cryptographic keys (AES-256, ChaCha20, Ed25519).",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string", "description": "Absolute path to binary file" },
                                "unit_bits": { "type": "integer", "description": "Unit size in bits (default: 8)" },
                                "skip_bits": { "type": "integer", "description": "Bits to skip before stream processing" },
                                "skip_units": { "type": "integer", "description": "Units to skip before stream processing" },
                                "pattern": { "type": "string", "description": "Pattern or stream pattern specification" },
                                "probe_field": { "type": "integer", "description": "Tuple field index to probe when pattern unpacking" },
                                "key_search_bits": { "type": "integer", "description": "Target key candidate window size in bits (e.g. 128, 256, 512, default: 256)" },
                                "count": { "type": "integer", "description": "Maximum number of units to sample" }
                            },
                            "required": ["file_path"]
                        }
                    },
                    {
                        "name": "bdd_explain_pattern",
                        "description": "Verify and explain a bit pattern, computing bit offsets, byte alignments, and field types.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string", "description": "Pattern string to explain" }
                            },
                            "required": ["pattern"]
                        }
                    },
                    {
                        "name": "bdd_list_presets",
                        "description": "List all available built-in binary format presets.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "bdd_transcode",
                        "description": "Transcode binary payload between patterns, presets, or formats (e.g. hex to JSON, packing NVFP4 to FP16, or re-encoding container headers).",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "input_hex": { "type": "string", "description": "Raw input hex string to transcode" },
                                "file_path": { "type": "string", "description": "Absolute path to input binary file" },
                                "unit_bits": { "type": "integer", "description": "Size of each input unit in bits (e.g. 8, 16, 32)" },
                                "input_pattern": { "type": "string", "description": "Pattern describing input binary structure" },
                                "input_preset": { "type": "string", "description": "Preset name describing input structure" },
                                "output_pattern": { "type": "string", "description": "Pattern describing desired output structure" },
                                "output_preset": { "type": "string", "description": "Preset describing desired output structure" },
                                "output_format": { "type": "string", "description": "Output format: 'json', 'hex', 'bits', or 'tuples' (default: 'hex')" },
                                "manipulators": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Optional list of manipulations to apply, e.g. ['xor:0xFF', 'round:0,0.01']"
                                },
                                "count": { "type": "integer", "description": "Maximum number of records to transcode" }
                            }
                        }
                    },
                    {
                        "name": "bdd_generate",
                        "description": "Generate synthetic test vectors conforming to a bit pattern or preset (useful for mock binary streams, unit tests, and AI verification).",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "pattern": { "type": "string", "description": "Pattern describing record structure (e.g. '8U,16U,1F')" },
                                "preset": { "type": "string", "description": "Preset name (e.g. 'mp3-header', 'mpeg-ts')" },
                                "count": { "type": "integer", "description": "Number of records to generate (default: 1)" },
                                "source": { "type": "string", "description": "Generator source: 'counter' (default), 'random', 'zeros', or 'ones'" },
                                "output_format": { "type": "string", "description": "Output format: 'json' (default), 'hex', 'bits', or 'tuples'" }
                            }
                        }
                    }
                ]);
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": tools
                    }
                });
                writeln!(stdout, "{}", serde_json::to_string(&response).unwrap()).unwrap();
                stdout.flush().unwrap();
            }
            "tools/call" => {
                let params = request.get("params");
                let tool_name = params
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let args = params
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or(json!({}));

                let call_result = execute_tool(tool_name, &args);
                let response = match call_result {
                    Ok(text) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [
                                {
                                    "type": "text",
                                    "text": text
                                }
                            ]
                        }
                    }),
                    Err(err) => json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32603,
                            "message": err
                        }
                    }),
                };
                writeln!(stdout, "{}", serde_json::to_string(&response).unwrap()).unwrap();
                stdout.flush().unwrap();
            }
            _ => {
                if let Some(id_val) = id {
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "error": {
                            "code": -32601,
                            "message": format!("Method '{}' not found", method)
                        }
                    });
                    writeln!(stdout, "{}", serde_json::to_string(&response).unwrap()).unwrap();
                    stdout.flush().unwrap();
                }
            }
        }
    }
    Ok(())
}

fn execute_tool(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "bdd_list_presets" => {
            let list: Vec<_> = all_presets()
                .iter()
                .map(|p| {
                    json!({
                        "name": p.name,
                        "description": p.description,
                        "pattern": p.pattern,
                        "unit_bits": p.unit_bits,
                        "little_endian": p.little_endian
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&list).unwrap())
        }
        "bdd_explain_pattern" => {
            let pattern = args
                .get("pattern")
                .and_then(|p| p.as_str())
                .ok_or_else(|| "Missing required argument 'pattern'".to_string())?;
            let exp = explain_pattern(pattern).map_err(|e| e.to_string())?;
            Ok(format_explanation_json(&exp))
        }
        "bdd_probe" => {
            let file_path = args
                .get("file_path")
                .and_then(|p| p.as_str())
                .ok_or_else(|| "Missing required argument 'file_path'".to_string())?;
            let mut f = File::open(file_path).map_err(|e| format!("Failed to open file: {}", e))?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            let report = probe_buffer(&buf, file_path);
            Ok(format_probe_json(&report))
        }
        "bdd_probe_units" => {
            let file_path = args
                .get("file_path")
                .and_then(|p| p.as_str())
                .ok_or_else(|| "Missing required argument 'file_path'".to_string())?;

            let mut cli_args = vec![
                "bdd".to_string(),
                format!("--input-file={}", file_path),
                "--probe-units".to_string(),
                "--output-json".to_string(),
            ];

            if let Some(u) = args.get("unit_bits").and_then(|u| u.as_u64()) {
                cli_args.push(format!("--input-unit={}", u));
            }
            if let Some(sb) = args.get("skip_bits").and_then(|s| s.as_u64()) {
                cli_args.push(format!("--input-skip-bits={}", sb));
            }
            if let Some(su) = args.get("skip_units").and_then(|s| s.as_u64()) {
                cli_args.push(format!("--input-skip-units={}", su));
            }
            if let Some(pat) = args.get("pattern").and_then(|p| p.as_str()) {
                cli_args.push(format!("--input-pattern={}", pat));
            }
            if let Some(pf) = args.get("probe_field").and_then(|f| f.as_u64()) {
                cli_args.push(format!("--probe-field={}", pf));
            }
            if let Some(kb) = args.get("key_search_bits").and_then(|k| k.as_u64()) {
                cli_args.push(format!("--probe-keys={}", kb));
            }
            if let Some(count) = args.get("count").and_then(|c| c.as_u64()) {
                cli_args.push(format!("--count={}", count));
            }

            let cli = Cli::try_parse_from(&cli_args).map_err(|e| e.to_string())?;
            let config = validate_and_process(cli).map_err(|e| e.to_string())?;

            let buf = SharedBuffer::default();
            bdd::engine::run_pipeline_to_writer(config, buf.clone()).map_err(|e| e.to_string())?;
            let output_bytes = buf.0.lock().unwrap().clone();
            let output_str = String::from_utf8_lossy(&output_bytes).into_owned();
            Ok(output_str)
        }
        "bdd_slice" => {
            let file_path = args
                .get("file_path")
                .and_then(|p| p.as_str())
                .ok_or_else(|| "Missing required argument 'file_path'".to_string())?;

            let mut cli_args = vec![
                "bdd".to_string(),
                format!("--input-file={}", file_path),
                "--output-json".to_string(),
            ];

            if let Some(pat) = args.get("pattern").and_then(|p| p.as_str()) {
                cli_args.push(format!("--input-pattern={}", pat));
            }
            if let Some(preset) = args.get("preset").and_then(|p| p.as_str()) {
                cli_args.push(format!("--preset={}", preset));
            }
            if let Some(count) = args.get("count").and_then(|p| p.as_u64()) {
                cli_args.push(format!("--count={}", count));
            }
            if let Some(skip) = args.get("skip_units").and_then(|p| p.as_u64()) {
                cli_args.push(format!("--input-skip-units={}", skip));
            }
            if let Some(bits) = args.get("skip_bits").and_then(|p| p.as_u64()) {
                cli_args.push(format!("--input-skip-bits={}", bits));
            }
            if let Some(ru) = args.get("raw_unit").and_then(|p| p.as_u64()) {
                cli_args.push(format!("--input-raw-unit={}", ru));
            }
            if let Some(off) = args.get("offset").and_then(|p| p.as_u64()) {
                cli_args.push(format!("--input-offset={}", off));
            }
            if let Some(jf) = args.get("json_fields").and_then(|p| p.as_str()) {
                cli_args.push(format!("--json-fields={}", jf));
            }

            let cli = Cli::try_parse_from(&cli_args).map_err(|e| e.to_string())?;
            let config = validate_and_process(cli).map_err(|e| e.to_string())?;

            let buf = SharedBuffer::default();
            bdd::engine::run_pipeline_to_writer(config, buf.clone()).map_err(|e| e.to_string())?;
            let output_bytes = buf.0.lock().unwrap().clone();
            let output_str = String::from_utf8_lossy(&output_bytes).into_owned();
            Ok(output_str)
        }
        "bdd_transcode" => {
            let mut temp_file = None;
            let file_path = if let Some(hex_str) = args.get("input_hex").and_then(|h| h.as_str()) {
                let cleaned: String = hex_str.chars().filter(|c| c.is_ascii_hexdigit()).collect();
                let bytes = (0..cleaned.len())
                    .step_by(2)
                    .filter_map(|i| {
                        if i + 2 <= cleaned.len() {
                            u8::from_str_radix(&cleaned[i..i + 2], 16).ok()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<u8>>();
                let mut tf = tempfile::NamedTempFile::new().map_err(|e| e.to_string())?;
                tf.write_all(&bytes).map_err(|e| e.to_string())?;
                tf.flush().map_err(|e| e.to_string())?;
                let path = tf.path().to_str().unwrap().to_string();
                temp_file = Some(tf);
                path
            } else if let Some(fp) = args.get("file_path").and_then(|f| f.as_str()) {
                fp.to_string()
            } else {
                return Err("Either 'input_hex' or 'file_path' must be provided".to_string());
            };

            let mut cli_args = vec!["bdd".to_string(), format!("--input-file={}", file_path)];

            let input_pat = if let Some(ip) = args.get("input_pattern").and_then(|p| p.as_str()) {
                cli_args.push(format!("--input-pattern={}", ip));
                Some(ip.to_string())
            } else if let Some(pr) = args.get("input_preset").and_then(|p| p.as_str()) {
                cli_args.push(format!("--preset={}", pr));
                bdd::preset::find_preset(pr).map(|p| p.pattern.to_string())
            } else {
                None
            };

            if let Some(u) = args.get("unit_bits").and_then(|u| u.as_u64()) {
                cli_args.push(format!("--input-unit={}", u));
            }

            if let Some(op) = args.get("output_pattern").and_then(|p| p.as_str()) {
                cli_args.push(format!("--output-pattern={}", op));
            } else if let Some(opr) = args.get("output_preset").and_then(|p| p.as_str()) {
                if let Some(p) = bdd::preset::find_preset(opr) {
                    cli_args.push(format!("--output-pattern={}", p.pattern));
                } else {
                    return Err(format!("Unknown output preset '{}'", opr));
                }
            } else if let Some(ref ip) = input_pat {
                cli_args.push(format!("--output-pattern={}", ip));
            }

            if let Some(count) = args.get("count").and_then(|c| c.as_u64()) {
                cli_args.push(format!("--count={}", count));
            }

            let mut inline_manips = Vec::new();
            if let Some(manips) = args.get("manipulators").and_then(|m| m.as_array()) {
                for m in manips {
                    if let Some(s) = m.as_str() {
                        inline_manips.push(s.trim_start_matches('-').to_string());
                    }
                }
            }

            let out_fmt = args
                .get("output_format")
                .and_then(|f| f.as_str())
                .unwrap_or("hex");
            match out_fmt {
                "json" => cli_args.push("--output-json".to_string()),
                "bits" => cli_args.push("--output-bits".to_string()),
                "tuples" => cli_args.push("--output-tuples".to_string()),
                _ => cli_args.push("--output-hex".to_string()),
            }

            let cli = Cli::try_parse_from(&cli_args).map_err(|e| e.to_string())?;
            let mut config = validate_and_process(cli).map_err(|e| e.to_string())?;
            config.raw_args = cli_args;
            config.inline_manipulators.extend(inline_manips);

            let buf = SharedBuffer::default();
            bdd::engine::run_pipeline_to_writer(config, buf.clone()).map_err(|e| e.to_string())?;
            drop(temp_file);
            let output_bytes = buf.0.lock().unwrap().clone();
            let output_str = String::from_utf8_lossy(&output_bytes).into_owned();
            Ok(output_str.trim_end().to_string())
        }
        "bdd_generate" => {
            let mut cli_args = vec!["bdd".to_string()];

            let source = args
                .get("source")
                .and_then(|s| s.as_str())
                .unwrap_or("counter");
            match source {
                "random" => cli_args.push("--input-random".to_string()),
                "zeros" => cli_args.push("--input-zeros".to_string()),
                "ones" => cli_args.push("--input-ones".to_string()),
                _ => cli_args.push("--input-counter".to_string()),
            }

            let count = args.get("count").and_then(|c| c.as_u64()).unwrap_or(1);
            cli_args.push(format!("--count={}", count));

            if let Some(p) = args.get("pattern").and_then(|p| p.as_str()) {
                cli_args.push(format!("--input-pattern={}", p));
            } else if let Some(pr) = args.get("preset").and_then(|p| p.as_str()) {
                cli_args.push(format!("--preset={}", pr));
            }

            let out_fmt = args
                .get("output_format")
                .and_then(|f| f.as_str())
                .unwrap_or("json");
            match out_fmt {
                "hex" => cli_args.push("--output-hex".to_string()),
                "bits" => cli_args.push("--output-bits".to_string()),
                "tuples" => cli_args.push("--output-tuples".to_string()),
                _ => cli_args.push("--output-json".to_string()),
            }

            let cli = Cli::try_parse_from(&cli_args).map_err(|e| e.to_string())?;
            let mut config = validate_and_process(cli).map_err(|e| e.to_string())?;
            config.raw_args = cli_args;

            let buf = SharedBuffer::default();
            bdd::engine::run_pipeline_to_writer(config, buf.clone()).map_err(|e| e.to_string())?;
            let output_bytes = buf.0.lock().unwrap().clone();
            let output_str = String::from_utf8_lossy(&output_bytes).into_owned();
            Ok(output_str.trim_end().to_string())
        }
        _ => Err(format!("Unknown tool '{}'", name)),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = run_mcp_server() {
        eprintln!("{}", e);
        std::process::exit(e.exit_code());
    }
    Ok(())
}
