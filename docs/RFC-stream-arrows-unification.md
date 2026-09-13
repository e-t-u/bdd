# RFC: Stream Arrow Pipeline Unification (Special Sources, Manipulators & Sinks)

- **Status**: Proposed / Roadmap Architecture
- **Target Version**: `bdd` v0.6.0
- **Author**: Esa Turtiainen & DeepMind Advanced Agentic Coding Team
- **Date**: September 2026

---

## 1. Executive Summary

`bdd` has unified container slicing and output framing using the **Stream Arrow Operator** (`->`), and recently introduced multi-stage inline unit transformations (`<in> -> <manip...> -> <out>`).

However, stream endpoints—such as **sources** (`--input-zeros`, `--input-random`, `--input-counter`, `--input-netlink`, `--input-file`) and **sinks** (`--output-json`, `--output-hex`, `--output-bits`, `--output-csv`, `--output-tuples`, `--output-visual`, `--output-file`)—are still configured via separate command-line flags.

This RFC proposes the **Complete Stream Arrow Unification**: allowing entire end-to-end dataflow pipelines—from synthetic or kernel sources, through sub-byte bit slicers and in-flight transformations, to structured JSON/hex sinks—to be expressed as a single, intuitive, human-readable pipeline string:

```bash
# Synthetic zero source, 8-bit slicer, bitwise XOR, to JSON sink:
bdd "zeros -> 8 -> xor(0xFF) -> json" --count 4

# Linux kernel process lifecycle telemetry directly to filtered JSON:
bdd "netlink -> proc-event -> filter(what == 2) -> json"

# Cryptographic test vector generator directly to hex words:
bdd "rand -> 256 -> hex" --count 10

# Complete file-to-file container slicing and PID extraction:
bdd "file('broadcast.ts') -> 188B[11:13] -> 13 -> file('pids.bin')"
```

---

## 2. Motivation & Design Goals

1. **Cognitive Ergonomics**: Users read pipelines from left to right: *Source* $\to$ *Extract* $\to$ *Transform* $\to$ *Repack* $\to$ *Sink*. Splitting the source into `-0` or `-c`, the pattern into positional arguments, and the sink into `--output-json` forces mental context switching across disparate CLI flags.
2. **Scripting Simplicity**: A single string encapsulates the complete dataflow definition. This makes aliases, shell functions, configuration files, and AI agent prompts clean, self-contained, and deterministic.
3. **Preserving 100% Backwards Compatibility**: Traditional positional invocations (`bdd "8 -> 3" < in > out`, `bdd 4U4U 4U4U < in > out`) and explicit CLI flags (`--input-zeros`, `--output-json`) must continue to work identically. The new source and sink tokens are seamless extensions to the arrow grammar.

---

## 3. Detailed Syntax Specification

A unified stream description consists of 2 or more stages connected by arrows (`->`):

```
[Source] -> <Slicer / InPattern> [-> Manipulator ...]* [-> Repacker / OutPattern] [-> Sink]
```

### 3.1 Source Tokens (Pipeline Inception)

When the first stage matches a recognized Source Token, `bdd` binds that stream source directly:

| Source Token | Equivalent CLI Flag | Description | Examples |
|---|---|---|---|
| `stdin` / `-` | (default stdin) | Standard input stream | `stdin -> 8 -> hex` |
| `zeros` / `zero` | `--input-zeros` / `-0` | Continuous stream of zero bits | `zeros -> 16 -> hex` |
| `ones` / `one` | `--input-ones` / `-1` | Continuous stream of one bits | `ones -> 8 -> xor(0x55) -> hex` |
| `rand` / `random` | `--input-random` / `-r` | Cryptographic pseudorandom bits | `rand -> 256 -> hex` |
| `counter` | `--input-counter` / `-c` | Auto-incrementing unsigned integer | `counter -> 16 -> hex` |
| `counter(START, STEP)` | `-c` with offset/step | Counter with custom start and increment | `counter(1000, 10) -> 16 -> hex` |
| `netlink` / `netlink:proc` | `--input-netlink` | Native Linux AF_NETLINK process connector | `netlink -> proc-event -> json` |
| `tuples` / `tuples(PATH)` | `--input-tuples` / `-t` | Comma-separated text tuple lines | `tuples -> 8U,8U -> raw` |
| `file(PATH)` / `'PATH'` | `--input-file=PATH` | Reads directly from specified filesystem path | `file('data.raw') -> 8 -> hex` |

*Default Rule*: If the first stage is not a recognized Source Token (e.g. `8`, `188B[11:13]`, `4U4U`), it represents the **Slicer / Input Pattern**, and data is read from `stdin` or `--input-file`.

---

### 3.2 Slicer & Input Pattern Tokens

Defines how raw bits are accumulated and unpacked into fields:
- **Bit Widths**: `8`, `16`, `32`, `64`, `12B`, `1MiB`
- **Physical Container (Form A)**: `188B[11:13]`, `8[2:4]`, `32[0:16]`
- **Physical Container (Form B)**: `[2:4:2]`, `[0:16:16]`
- **Pattern Specifiers**: `8U,8U`, `sync:11U,ver:2U`, `4E4E`, `16S`
- **Protocol Presets**: `mpeg-ts`, `ipv4-header`, `udp-header`, `proc-pagemap`, `proc-event`

---

### 3.3 Inline Manipulator Tokens

Intermediate stages between the slicer and repacker/sink:
- **Bitwise**: `xor(VAL)`, `and(VAL)`, `or(VAL)`, `not`
- **Arithmetic**: `add(VAL)`, `sub(VAL)`, `mul(VAL)`, `div(VAL)`, `mod(VAL)`, `abs`, `sign`
- **Shifts**: `shift_left(BITS)`, `shift_right(BITS)`
- **Bounds & Quantization**: `clamp(MIN, MAX:MODE)`, `round(STEP:MODE)`
- **Predicates & Filtering**: `filter(FIELD, OP, VAL)` or `filter(EXPR)` (e.g. `filter(what == 2)`)
- **Field Reordering**: `rearrange(1, 0, 2)`

*Field Addressing*: Manipulators accept either a single argument defaulting to field 0 (e.g. `xor(0xFF)`), or explicit field indexing (e.g. `xor(1, 0xFF)`).

---

### 3.4 Sink Tokens (Pipeline Termination)

When the final stage matches a recognized Sink Token, `bdd` directs the output stream to the appropriate formatter or destination:

| Sink Token | Equivalent CLI Flag | Output Behavior |
|---|---|---|
| `stdout` | (default stdout) | Binary unit stream to stdout |
| `raw` / `bin` / `binary` | `--output-file` / default | Raw unpacked/packed binary bytes |
| `json` / `ndjson` | `--output-json` | Newline-delimited JSON tuples (`{"fields":[...]}`) |
| `json:object` | `--output-json --json-object` | Keyed JSON objects using field names |
| `hex` | `--output-hex` / `-x` | Formatted zero-padded hexadecimal words |
| `bits` | `--output-bits` / `-b` | ASCII bit strings ('0' and '1') |
| `csv` | `--output-csv` | Standard comma-separated values |
| `tuples` | `--output-tuples` / `-T` | Space/comma-separated tuple values |
| `visual` | `--output-visual` | Colorized ANSI terminal matrix |
| `file(PATH)` | `--output-file=PATH` | Writes output directly to specified file path |

*Default Rule*: If the final stage is a numeric width or pattern (e.g. `-> 8`, `-> 188B[11:13]`), it represents the **Repacker / Output Framing Specifier**, and formatted output defaults to binary or explicit CLI flags.

---

## 4. Real-World End-to-End Examples

### 4.1 Synthetic Test Vector Generation
```bash
# Generate 4 words of alternating test bit patterns in JSON:
bdd "zeros -> 8 -> xor(0xAA) -> json" --count 4
# Output:
# {"fields":[170],"unit":0}
# {"fields":[170],"unit":1}
# {"fields":[170],"unit":2}
# {"fields":[170],"unit":3}
```

### 4.2 Linux Kernel Event Telemetry Stream
```bash
# Listen directly to kernel process events, filter for EXEC, and emit JSON:
sudo bdd "netlink -> proc-event -> filter(what == 2) -> json"
```

### 4.3 Embedded ADC Calibration to Hex Dump
```bash
# 12-bit ADC raw stream -> DC offset subtraction -> Gain -> Clamp -> 8-bit DAC -> Hex words:
bdd "file('sensor.raw') -> 12 -> sub(512) -> mul(2) -> clamp(0,0,255:saturate) -> 8 -> hex"
```

### 4.4 In-Place Broadcast Video Scrambler
```bash
# Slices MPEG-TS payload, applies XOR mask, repacks into container, writes to file:
bdd "file('in.ts') -> 188B[32:1472] -> xor(0xA5) -> 188B[32:1472] -> file('scrambled.ts')"
```

---

## 5. Implementation Roadmap for v0.6.0

1. **Parser Extension (`src/stream_pattern.rs`)**:
   - Update `parse_stream_io_pattern` to tokenize stages on `->`.
   - Inspect Stage 0: if matching known source names (`zeros`, `ones`, `rand`, `counter`, `netlink`, `tuples`, `file(...)`), extract source configuration into `StreamIoPattern.source`.
   - Inspect Stage $N-1$: if matching known sink names (`json`, `hex`, `bits`, `csv`, `tuples`, `visual`, `raw`, `file(...)`), extract sink configuration into `StreamIoPattern.sink`.
   - Remaining stages are parsed as: Slicer $\to$ [Manipulator]* $\to$ [Repacker].
2. **CLI & Engine Integration (`src/cli.rs`, `src/engine.rs`)**:
   - Merge `StreamIoPattern.source` into `ValidatedConfig.input_*` flags if not explicitly set on CLI.
   - Merge `StreamIoPattern.sink` into `ValidatedConfig.output_*` flags if not explicitly set on CLI.
3. **Diagnostic Verification**:
   - Add unit tests in `src/stream_pattern.rs` for source/sink recognition.
   - Add integration tests verifying full pipelines (`zeros -> 8 -> hex`, `netlink -> proc-event -> json`).
