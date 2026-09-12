#!/usr/bin/env python3
"""
Python alternative server for bdd Web UI application.
Serves static assets from web/ and proxies /api endpoints to the bdd CLI binary.
Run with: python3 web/server.py [port]
"""

import http.server
import socketserver
import json
import os
import sys
import subprocess
import base64
import tempfile

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 7788
WEB_DIR = os.path.dirname(os.path.abspath(__file__))
REPO_DIR = os.path.dirname(WEB_DIR)
BDD_BIN = os.path.join(REPO_DIR, "target", "release", "bdd")
if not os.path.exists(BDD_BIN):
    BDD_BIN = os.path.join(REPO_DIR, "target", "debug", "bdd")
if not os.path.exists(BDD_BIN):
    BDD_BIN = "bdd"

class BddHttpHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=WEB_DIR, **kwargs)

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_cors_headers()
        self.end_headers()

    def do_GET(self):
        if self.path == "/api/status":
            self.send_json(200, {"status": "ok", "version": "0.5.0", "backend": "python-bridge"})
            return
        elif self.path == "/api/presets":
            cmd = [BDD_BIN, "--list-presets"]
            try:
                proc = subprocess.run(cmd, capture_output=True, text=True)
                self.send_json(200, {"raw": proc.stdout})
            except Exception as e:
                self.send_json(500, {"error": str(e)})
            return
        super().do_GET()

    def do_POST(self):
        content_length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_length) if content_length > 0 else b""
        
        if self.path == "/api/explain":
            try:
                data = json.loads(body.decode("utf-8"))
                pat = data.get("pattern", "")
                cmd = [BDD_BIN, f"--explain-pattern={pat}", "--output-json"]
                proc = subprocess.run(cmd, capture_output=True, text=True)
                if proc.returncode == 0:
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.send_cors_headers()
                    self.end_headers()
                    self.wfile.write(proc.stdout.encode("utf-8"))
                else:
                    self.send_json(400, {"error": proc.stderr})
            except Exception as e:
                self.send_json(400, {"error": str(e)})
            return

        elif self.path == "/api/probe":
            try:
                data = json.loads(body.decode("utf-8"))
                b64 = data.get("file_base64")
                target = data.get("target")
                temp_file = None
                
                if b64:
                    raw_bytes = base64.b64decode(b64)
                    temp_file = tempfile.NamedTemporaryFile(delete=False, prefix="bdd_probe_")
                    temp_file.write(raw_bytes)
                    temp_file.close()
                    target_path = temp_file.name
                elif target:
                    if os.path.exists(target):
                        target_path = target
                    else:
                        target_path = os.path.join(REPO_DIR, "contrib", "data", target)
                else:
                    target_path = os.path.join(REPO_DIR, "contrib", "data", "sample.mp3")

                cmd = [BDD_BIN, f"--probe={target_path}", "--output-json"]
                proc = subprocess.run(cmd, capture_output=True, text=True)
                
                if temp_file and os.path.exists(temp_file.name):
                    os.unlink(temp_file.name)

                if proc.returncode == 0:
                    rep = json.loads(proc.stdout)
                    self.send_json(200, {"success": True, "report": rep})
                else:
                    self.send_json(400, {"success": False, "error": proc.stderr})
            except Exception as e:
                self.send_json(500, {"success": False, "error": str(e)})
            return

        elif self.path == "/api/process":
            try:
                data = json.loads(body.decode("utf-8"))
                args = data.get("args", [])
                b64 = data.get("file_base64")
                tuples_text = data.get("tuples_text")
                is_binary = data.get("sink") == "binary"
                
                temp_in = None
                temp_out = None
                
                if b64:
                    raw = base64.b64decode(b64)
                    temp_in = tempfile.NamedTemporaryFile(delete=False, prefix="bdd_web_in_")
                    temp_in.write(raw)
                    temp_in.close()

                if is_binary:
                    temp_out = tempfile.NamedTemporaryFile(delete=False, prefix="bdd_web_out_")
                    temp_out.close()

                cmd = [BDD_BIN]
                file_arg_added = False
                
                for a in args:
                    if a.startswith("--input-file="):
                        if temp_in:
                            cmd.append(f"--input-file={temp_in.name}")
                            file_arg_added = True
                            continue
                    cmd.append(a)

                if temp_in and not file_arg_added:
                    cmd.append(f"--input-file={temp_in.name}")

                if temp_out:
                    cmd.append(f"--output-file={temp_out.name}")

                stdin_data = tuples_text.encode("utf-8") if tuples_text else None
                proc = subprocess.run(cmd, input=stdin_data, capture_output=True, cwd=REPO_DIR)

                binary_b64 = None
                if temp_out and os.path.exists(temp_out.name):
                    with open(temp_out.name, "rb") as f:
                        binary_b64 = base64.b64encode(f.read()).decode("ascii")
                    os.unlink(temp_out.name)

                if temp_in and os.path.exists(temp_in.name):
                    os.unlink(temp_in.name)

                res = {
                    "success": proc.returncode == 0,
                    "stdout": proc.stdout.decode("utf-8", errors="replace"),
                    "stderr": proc.stderr.decode("utf-8", errors="replace"),
                    "binary_base64": binary_b64,
                    "exit_code": proc.returncode
                }
                self.send_json(200, res)
            except Exception as e:
                self.send_json(500, {"success": False, "stderr": str(e), "exit_code": 1})
            return

        self.send_error(404, "Unknown endpoint")

    def send_json(self, code, obj):
        payload = json.dumps(obj).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_cors_headers()
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def send_cors_headers(self):
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, Authorization")

if __name__ == "__main__":
    with socketserver.TCPServer(("", PORT), BddHttpHandler) as httpd:
        print(f"⚡ bdd Python Web UI running on http://localhost:{PORT}")
        print(f"Serving files from: {WEB_DIR}")
        print("Press Ctrl+C to stop.")
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            print("\nServer stopped.")
