//! Decoupled standalone Web UI application server for bdd.
//!
//! Exclusively uses the `bdd` CLI executable via subprocess execution.
//! Has zero dependencies on bdd library internals.

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

static REQ_COUNTER: AtomicU64 = AtomicU64::new(1);

const EMBEDDED_INDEX_HTML: &str = include_str!("../index.html");
const EMBEDDED_STYLE_CSS: &str = include_str!("../style.css");
const EMBEDDED_APP_JS: &str = include_str!("../app.js");

#[derive(Deserialize)]
struct ProcessRequest {
    #[serde(default)]
    args: Vec<String>,
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
    file_base64: Option<String>,
    #[serde(default)]
    target: Option<String>,
}

/// Locate the bdd CLI binary
fn find_bdd_binary() -> PathBuf {
    if let Ok(path) = env::var("BDD_BIN") {
        let p = PathBuf::from(path);
        if p.exists() {
            return p;
        }
    }

    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            // Check relative to target/debug or target/release
            let sibling_bdd = parent.join("bdd");
            if sibling_bdd.exists() {
                return sibling_bdd;
            }
        }
    }

    // Check project target directory
    for rel in &[
        "target/release/bdd",
        "target/debug/bdd",
        "../target/release/bdd",
        "../target/debug/bdd",
    ] {
        let p = PathBuf::from(rel);
        if p.exists() {
            return p;
        }
    }

    PathBuf::from("bdd")
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let port: u16 = if args.len() > 1 {
        args[1].parse().unwrap_or(7788)
    } else {
        7788
    };

    let bdd_bin = Arc::new(find_bdd_binary());
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to {}: {}", addr, e);
            std::process::exit(1);
        }
    };

    println!(
        "⚡ bdd Standalone Rust Web UI running on http://localhost:{}",
        port
    );
    println!("   Using bdd CLI binary at: {}", bdd_bin.display());
    println!("   Press Ctrl+C to stop.");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let bin = Arc::clone(&bdd_bin);
                thread::spawn(move || {
                    handle_connection(stream, &bin);
                });
            }
            Err(e) => {
                eprintln!("Connection failed: {}", e);
            }
        }
    }
}

fn handle_connection(stream: TcpStream, bdd_bin: &Path) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();

    if reader.read_line(&mut request_line).is_err() || request_line.is_empty() {
        return;
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }

    let method = parts[0];
    let raw_path = parts[1];
    let path = raw_path.split('?').next().unwrap_or("/");

    let mut content_length = 0usize;
    loop {
        let mut header_line = String::new();
        if reader.read_line(&mut header_line).is_err()
            || header_line == "\r\n"
            || header_line.is_empty()
        {
            break;
        }
        let lower = header_line.to_lowercase();
        if lower.starts_with("content-length:") {
            if let Some(val) = header_line.split(':').nth(1) {
                content_length = val.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        let _ = reader.read_exact(&mut body);
    }

    if method == "OPTIONS" {
        send_response(&stream, "204 No Content", "text/plain", b"");
        return;
    }

    match (method, path) {
        ("GET", "/") | ("GET", "/index.html") => {
            let content = load_asset("web/index.html", EMBEDDED_INDEX_HTML);
            send_response(
                &stream,
                "200 OK",
                "text/html; charset=utf-8",
                content.as_bytes(),
            );
        }
        ("GET", "/style.css") => {
            let content = load_asset("web/style.css", EMBEDDED_STYLE_CSS);
            send_response(
                &stream,
                "200 OK",
                "text/css; charset=utf-8",
                content.as_bytes(),
            );
        }
        ("GET", "/app.js") => {
            let content = load_asset("web/app.js", EMBEDDED_APP_JS);
            send_response(
                &stream,
                "200 OK",
                "application/javascript; charset=utf-8",
                content.as_bytes(),
            );
        }
        ("GET", "/api/status") => {
            let json = serde_json::json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION"),
                "backend": "standalone-rust",
                "cli": bdd_bin.to_string_lossy()
            });
            send_response(
                &stream,
                "200 OK",
                "application/json",
                json.to_string().as_bytes(),
            );
        }
        ("GET", "/api/presets") => match Command::new(bdd_bin).arg("--list-presets").output() {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                let json = serde_json::json!({ "raw": text });
                send_response(
                    &stream,
                    "200 OK",
                    "application/json",
                    json.to_string().as_bytes(),
                );
            }
            Err(e) => {
                let err = serde_json::json!({ "error": format!("Failed to run bdd: {}", e) });
                send_response(
                    &stream,
                    "500 Internal Server Error",
                    "application/json",
                    err.to_string().as_bytes(),
                );
            }
        },
        ("POST", "/api/explain") => {
            if let Ok(req) = serde_json::from_slice::<ExplainRequest>(&body) {
                let out = Command::new(bdd_bin)
                    .arg(format!("--explain-pattern={}", req.pattern))
                    .arg("--output-json")
                    .output();
                match out {
                    Ok(o) if o.status.success() => {
                        send_response(&stream, "200 OK", "application/json", &o.stdout);
                    }
                    Ok(o) => {
                        let err = String::from_utf8_lossy(&o.stderr).to_string();
                        let json = serde_json::json!({ "error": err });
                        send_response(
                            &stream,
                            "400 Bad Request",
                            "application/json",
                            json.to_string().as_bytes(),
                        );
                    }
                    Err(e) => {
                        let json =
                            serde_json::json!({ "error": format!("Subprocess error: {}", e) });
                        send_response(
                            &stream,
                            "500 Internal Server Error",
                            "application/json",
                            json.to_string().as_bytes(),
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
                        format!("contrib/data/{}", t)
                    }
                } else {
                    "contrib/data/sample.mp3".to_string()
                };

                let out = Command::new(bdd_bin)
                    .arg(format!("--probe={}", target_file))
                    .arg("--output-json")
                    .output();

                if let Some(tp) = temp_path {
                    let _ = fs::remove_file(tp);
                }

                match out {
                    Ok(o) if o.status.success() => {
                        if let Ok(rep) = serde_json::from_slice::<serde_json::Value>(&o.stdout) {
                            let json = serde_json::json!({ "success": true, "report": rep });
                            send_response(
                                &stream,
                                "200 OK",
                                "application/json",
                                json.to_string().as_bytes(),
                            );
                        } else {
                            send_response(&stream, "200 OK", "application/json", &o.stdout);
                        }
                    }
                    Ok(o) => {
                        let err = String::from_utf8_lossy(&o.stderr).to_string();
                        let json = serde_json::json!({ "success": false, "error": err });
                        send_response(
                            &stream,
                            "400 Bad Request",
                            "application/json",
                            json.to_string().as_bytes(),
                        );
                    }
                    Err(e) => {
                        let json = serde_json::json!({ "success": false, "error": format!("Subprocess error: {}", e) });
                        send_response(
                            &stream,
                            "500 Internal Server Error",
                            "application/json",
                            json.to_string().as_bytes(),
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
        ("POST", "/api/process") => {
            handle_api_process(&stream, &body, bdd_bin);
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

fn load_asset(rel_path: &str, embedded: &str) -> String {
    if let Ok(content) = fs::read_to_string(rel_path) {
        return content;
    }
    let alt = format!("../{}", rel_path);
    if let Ok(content) = fs::read_to_string(&alt) {
        return content;
    }
    embedded.to_string()
}

fn handle_api_process(stream: &TcpStream, body: &[u8], bdd_bin: &Path) {
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

    let mut cmd = Command::new(bdd_bin);
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
