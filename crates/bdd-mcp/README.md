# bdd-mcp

Model Context Protocol (MCP) server companion for the `bdd` bitstream manipulation engine.

Provides a standard JSON-RPC 2.0 stdio server enabling AI models and agents
(Antigravity, Claude Desktop, Cursor) to slice, unpack, inspect, probe, transcode,
and synthesize binary streams natively via tool calls:
- `bdd_slice`: Unpack unaligned bitstream fields into JSON records.
- `bdd_probe`: Analyze Shannon entropy and periodic strides.
- `bdd_probe_units`: Probe unit stream entropy and detect cryptographic keys.
- `bdd_explain_pattern`: Verify pattern offsets, types, and alignment.
- `bdd_list_presets`: Enumerate all built-in binary presets.
- `bdd_transcode`: Transcode binary between patterns, formats, and manipulators.
- `bdd_generate`: Synthesize test vectors conforming to a bit pattern or preset.

## Usage

Run directly via stdio:
```bash
bdd-mcp
```
Or with cargo:
```bash
cargo run -p bdd-mcp
```
