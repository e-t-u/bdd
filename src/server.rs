//! Embedded lightweight HTTP web server for the bdd Web UI application.
//!
//! Provides a single-binary zero-dependency browser application to upload binary files,
//! select presets, configure bitstreams, visualize patterns, and execute slicing pipelines.

use crate::explain::{explain_pattern, format_explanation_json};
use crate::preset::all_presets;
use crate::probe::{format_probe_json, probe_reader};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

static REQ_COUNTER: AtomicU64 = AtomicU64::new(1);

const EMBEDDED_INDEX_HTML: &str = include_str!("../web/index.html");
const EMBEDDED_STYLE_CSS: &str = include_str!("../web/style.css");
const EMBEDDED_APP_JS: &str = include_str!("../web/app.js");

#[derive(Deserialize)]
#[allow(dead_code)]
struct ProcessRequest {
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    file_name: Option<String>,
    #[serde(default)]
    file_base64: Option<String>,
    #[serde(default)]
    tuples_text: Option<String>,
    #[serde(default)]
    sink: Option<String>,
}

#[derive(Serialize)]
struct ProcessResponse {
    success: bool,
    stdout: String,
    stderr: String,
    binary_base64: Option<String>,
    exit_code: i32,
}

#[derive(Deserialize)]
struct ExplainRequest {
    pattern: String,
}

#[derive(Deserialize)]
struct ProbeRequest {
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    file_base64: Option<String>,
}

/// Run the embedded HTTP web server on the specified port.
pub fn run_server(port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let bind_addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&bind_addr)?;

    println!("============================================================");
    println!(" ⚡ bdd Web UI Application Server Running");
    println!("============================================================");
    println!("  URL: http://localhost:{}/", port);
    println!("  URL: http://127.0.0.1:{}/", port);
    println!("  Bound: {}", bind_addr);
    println!("  Press Ctrl+C to stop.");
    println!("============================================================");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    handle_connection(stream);
                });
            }
            Err(e) => {
                eprintln!("[bdd web] Connection failed: {}", e);
            }
        }
    }

    Ok(())
}

fn handle_connection(mut stream: TcpStream) {
    let mut reader = BufReader::new(&mut stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let path = parts[1];

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }
        if let Some(val) = trimmed.strip_prefix("Content-Length:") {
            content_length = val.trim().parse().unwrap_or(0);
        } else if let Some(val) = trimmed.strip_prefix("content-length:") {
            content_length = val.trim().parse().unwrap_or(0);
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 && reader.read_exact(&mut body).is_err() {
        send_response(
            &stream,
            "400 Bad Request",
            "application/json",
            br#"{"error":"Incomplete body"}"#,
        );
        return;
    }

    if method == "OPTIONS" {
        send_response(&stream, "204 No Content", "text/plain", b"");
        return;
    }

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            let content =
                fs::read_to_string("web/index.html").unwrap_or_else(|_| EMBEDDED_INDEX_HTML.into());
            send_response(
                &stream,
                "200 OK",
                "text/html; charset=utf-8",
                content.as_bytes(),
            );
        }
        ("GET", "/style.css") => {
            let content =
                fs::read_to_string("web/style.css").unwrap_or_else(|_| EMBEDDED_STYLE_CSS.into());
            send_response(
                &stream,
                "200 OK",
                "text/css; charset=utf-8",
                content.as_bytes(),
            );
        }
        ("GET", "/app.js") => {
            let content =
                fs::read_to_string("web/app.js").unwrap_or_else(|_| EMBEDDED_APP_JS.into());
            send_response(
                &stream,
                "200 OK",
                "application/javascript; charset=utf-8",
                content.as_bytes(),
            );
        }
        ("GET", "/api/status") => {
            send_response(
                &stream,
                "200 OK",
                "application/json",
                br#"{"status":"ok","version":"0.3.0"}"#,
            );
        }
        ("GET", "/api/presets") => {
            let presets = all_presets();
            let json = serde_json::json!(presets
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "name": p.name,
                        "description": p.description,
                        "pattern": p.pattern,
                        "unit_bits": p.unit_bits,
                        "little_endian": p.little_endian,
                    })
                })
                .collect::<Vec<_>>());
            send_response(
                &stream,
                "200 OK",
                "application/json",
                json.to_string().as_bytes(),
            );
        }
        ("POST", "/api/explain") => {
            if let Ok(req) = serde_json::from_slice::<ExplainRequest>(&body) {
                match explain_pattern(&req.pattern) {
                    Ok(exp) => {
                        let json = format_explanation_json(&exp);
                        send_response(&stream, "200 OK", "application/json", json.as_bytes());
                    }
                    Err(e) => {
                        let err_json = serde_json::json!({"error": e.to_string()});
                        send_response(
                            &stream,
                            "400 Bad Request",
                            "application/json",
                            err_json.to_string().as_bytes(),
                        );
                    }
                }
            } else {
                send_response(
                    &stream,
                    "400 Bad Request",
                    "application/json",
                    br#"{"error":"Invalid JSON"}"#,
                );
            }
        }
        ("POST", "/api/probe") => {
            if let Ok(req) = serde_json::from_slice::<ProbeRequest>(&body) {
                let mut temp_path: Option<String> = None;
                let target_file = if let Some(ref b64) = req.file_base64 {
                    let bytes = base64_decode(b64).unwrap_or_default();
                    let tpath = format!(
                        "/tmp/bdd_probe_{}_{}.bin",
                        std::process::id(),
                        REQ_COUNTER.fetch_add(1, Ordering::SeqCst)
                    );
                    let _ = fs::write(&tpath, bytes);
                    temp_path = Some(tpath.clone());
                    tpath
                } else if let Some(ref t) = req.target {
                    if Path::new(t).exists() {
                        t.clone()
                    } else {
                        let candidate = format!("contrib/data/{}", t);
                        if Path::new(&candidate).exists() {
                            candidate
                        } else {
                            t.clone()
                        }
                    }
                } else {
                    "contrib/data/sample.mp3".to_string()
                };

                let res = match File::open(&target_file) {
                    Ok(mut f) => match probe_reader(&mut f, &target_file) {
                        Ok(rep) => {
                            let json = format_probe_json(&rep);
                            format!(r#"{{"success":true,"report":{}}}"#, json)
                        }
                        Err(e) => format!(r#"{{"success":false,"error":"{}"}}"#, e),
                    },
                    Err(e) => format!(r#"{{"success":false,"error":"Cannot open file: {}"}}"#, e),
                };

                if let Some(tp) = temp_path {
                    let _ = fs::remove_file(tp);
                }

                send_response(&stream, "200 OK", "application/json", res.as_bytes());
            } else {
                send_response(
                    &stream,
                    "400 Bad Request",
                    "application/json",
                    br#"{"error":"Invalid JSON"}"#,
                );
            }
        }
        ("POST", "/api/process") => {
            handle_api_process(&stream, &body);
        }
        _ => {
            send_response(
                &stream,
                "404 Not Found",
                "application/json",
                br#"{"error":"Endpoint not found"}"#,
            );
        }
    }
}

fn handle_api_process(stream: &TcpStream, body: &[u8]) {
    let req: ProcessRequest = match serde_json::from_slice(body) {
        Ok(r) => r,
        Err(e) => {
            let err =
                serde_json::json!({ "success": false, "stderr": format!("JSON error: {}", e) });
            send_response(
                stream,
                "400 Bad Request",
                "application/json",
                err.to_string().as_bytes(),
            );
            return;
        }
    };

    let mut temp_input_path: Option<String> = None;
    let mut temp_output_path: Option<String> = None;

    if let Some(ref b64) = req.file_base64 {
        let bytes = base64_decode(b64).unwrap_or_default();
        let tpath = format!(
            "/tmp/bdd_web_in_{}_{}.bin",
            std::process::id(),
            REQ_COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        let _ = fs::write(&tpath, bytes);
        temp_input_path = Some(tpath);
    }

    let is_binary_sink = req.sink.as_deref() == Some("binary");
    if is_binary_sink {
        let opath = format!(
            "/tmp/bdd_web_out_{}_{}.bin",
            std::process::id(),
            REQ_COUNTER.fetch_add(1, Ordering::SeqCst)
        );
        temp_output_path = Some(opath);
    }

    let current_exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "./bdd".into());

    let mut cmd = Command::new(current_exe);
    let mut stdin_content: Option<String> = req.tuples_text;

    let mut file_arg_added = false;
    for arg in &req.args {
        if arg.starts_with("--input-file=") {
            if let Some(ref tip) = temp_input_path {
                cmd.arg(format!("--input-file={}", tip));
                file_arg_added = true;
                continue;
            }
        }
        cmd.arg(arg);
    }

    if !file_arg_added {
        if let Some(ref tip) = temp_input_path {
            cmd.arg(format!("--input-file={}", tip));
        }
    }

    if let Some(ref top) = temp_output_path {
        cmd.arg(format!("--output-file={}", top));
    }

    if stdin_content.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let resp = ProcessResponse {
                success: false,
                stdout: String::new(),
                stderr: format!("Failed to spawn bdd process: {}", e),
                binary_base64: None,
                exit_code: 1,
            };
            let json = serde_json::to_string(&resp).unwrap();
            send_response(stream, "200 OK", "application/json", json.as_bytes());
            if let Some(p) = temp_input_path {
                let _ = fs::remove_file(p);
            }
            return;
        }
    };

    if let Some(content) = stdin_content.take() {
        if let Some(mut cin) = child.stdin.take() {
            let _ = cin.write_all(content.as_bytes());
        }
    }

    let output = match child.wait_with_output() {
        Ok(o) => o,
        Err(e) => {
            let resp = ProcessResponse {
                success: false,
                stdout: String::new(),
                stderr: format!("Process execution error: {}", e),
                binary_base64: None,
                exit_code: 1,
            };
            let json = serde_json::to_string(&resp).unwrap();
            send_response(stream, "200 OK", "application/json", json.as_bytes());
            if let Some(p) = temp_input_path {
                let _ = fs::remove_file(p);
            }
            return;
        }
    };

    let mut binary_base64 = None;
    if let Some(ref top) = temp_output_path {
        if let Ok(out_bytes) = fs::read(top) {
            binary_base64 = Some(base64_encode(&out_bytes));
        }
        let _ = fs::remove_file(top);
    }

    if let Some(tip) = temp_input_path {
        let _ = fs::remove_file(tip);
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

    let resp = ProcessResponse {
        success: output.status.success(),
        stdout: stdout_str,
        stderr: stderr_str,
        binary_base64,
        exit_code: output.status.code().unwrap_or(0),
    };

    let json = serde_json::to_string(&resp).unwrap();
    send_response(stream, "200 OK", "application/json", json.as_bytes());
}

fn send_response(mut stream: &TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, &b) in TABLE.iter().enumerate() {
        lookup[b as usize] = i as u8;
    }
    lookup[b'-' as usize] = 62;
    lookup[b'_' as usize] = 63;
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for &b in input.as_bytes() {
        if b == b'=' || b.is_ascii_whitespace() {
            continue;
        }
        let val = lookup[b as usize];
        if val == 255 {
            continue;
        }
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) & 63] as char);
        out.push(TABLE[(triple >> 12) & 63] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(triple >> 6) & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[triple & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}
