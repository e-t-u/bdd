# bdd Web UI Application Server

Interactive web interface for the `bdd` bitstream manipulation tool.

This server is decoupled from the core `bdd` Rust library and communicates **exclusively via the `bdd` command-line interface (`bdd` CLI)** through subprocess pipes. It does not depend on or link against `bdd` internal structures, providing maximum isolation, stability, and zero compile-time coupling with the core bitstream engine.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Browser Frontend                     │
│               (HTML5 / CSS3 / Vanilla JS)               │
└────────────────────────────┬────────────────────────────┘
                             │ HTTP REST API
                             ▼
┌─────────────────────────────────────────────────────────┐
│             Decoupled Web UI Server                     │
│    Option A: web/server.py (Python 3 stdlib, 0 deps)    │
│    Option B: web/src/main.rs (Rust bdd-web, min deps)   │
└────────────────────────────┬────────────────────────────┘
                             │ std::process::Command (CLI)
                             ▼
┌─────────────────────────────────────────────────────────┐
│                     bdd CLI Binary                      │
│            (--explain-pattern, --probe, etc.)           │
└─────────────────────────────────────────────────────────┘
```

---

## Quick Start

Ensure the `bdd` CLI binary is built first:
```bash
cargo build --release
```

### Option A: Run via Python (Zero external dependencies)
```bash
python3 web/server.py [port]
# Default port: 7788
# Access: http://localhost:7788
```

### Option B: Run via Standalone Rust Server
```bash
cargo run --manifest-path web/Cargo.toml -- [port]
# Or from inside web/:
# cd web && cargo run -- 7788
```

---

## API Endpoints

The decoupled server exposes the following endpoints to the frontend, delegating execution directly to the `bdd` CLI:

| Endpoint | Method | Description | Underlying CLI Execution |
|---|---|---|---|
| `/` | `GET` | Serves `index.html` UI application | Static file serve |
| `/api/status` | `GET` | Health check and backend status | Returns server status and CLI binary path |
| `/api/presets` | `GET` | List available protocol presets | `bdd --list-presets` |
| `/api/explain` | `POST` | Explain bitstream schema layout | `bdd --explain-pattern=<pattern> --output-json` |
| `/api/probe` | `POST` | Probe binary entropy and stride | `bdd --probe=<file> --output-json` |
| `/api/process` | `POST` | Run slicing, transcoding or extraction | `bdd [args...]` with piped stdin/binary output |

---

## Environment Variables

- `BDD_BIN`: Explicit path to the `bdd` executable (e.g. `/usr/local/bin/bdd` or `target/release/bdd`). Defaults to automatically checking relative build paths (`target/release/bdd`, `target/debug/bdd`) before falling back to system `PATH`.
