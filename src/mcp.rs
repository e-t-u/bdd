//! Model Context Protocol (MCP) server implementation for bdd.
//!
//! Provides a standard JSON-RPC 2.0 stdio server interface enabling AI models
//! and coding assistants (Antigravity, Claude, Cursor) to slice, unpack, inspect,
//! and probe binary streams natively via tool calls.

use crate::cli::{validate_and_process, Cli};
use crate::error::BddError;
use crate::explain::{explain_pattern, format_explanation_json};
use crate::preset::all_presets;
use crate::probe::{format_probe_json, probe_buffer};
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

pub fn run_mcp_server() -> Result<(), BddError> {
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

            // Capture output
            let buf = SharedBuffer::default();
            crate::engine::run_pipeline_to_writer(config, buf.clone())
                .map_err(|e| e.to_string())?;
            let output_bytes = buf.0.lock().unwrap().clone();
            let output_str = String::from_utf8_lossy(&output_bytes).into_owned();
            Ok(output_str)
        }
        _ => Err(format!("Unknown tool '{}'", name)),
    }
}
