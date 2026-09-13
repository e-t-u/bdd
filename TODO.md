# bdd Roadmap & Planned Enhancements

This document consolidates high-value feature improvements, API additions, and architectural enhancements planned for `bdd`. It supersedes the legacy `docs/TODO` from the 2010 Python prototype.

---

## 1. High Priority Enhancements

### 1.1 Quoted Number Strings in `--input-tuples` [COMPLETED]
- **Current Behavior**: Quoted numbers are strictly preserved as raw ASCII string bytes (`"5"` $\rightarrow$ ASCII `0x35` / `53`), while unquoted numbers (`5`) are parsed as numeric values (`0x05`).
- **Reference**: Resolves the note in `test/test.sh:151` (`echo -en "\"5\"" | ./bdd --input-tuples --output-hex` outputs `35`).

### 1.2 Overhaul of Rounding, Range Clamping, and Floating-Point Handling [COMPLETED]
- **Current Architecture**:
  - Decoupled upper-end range overflow handling (`--clamp FIELD,MIN,MAX[:MODE]` or `--clamp FIELD,LIMIT[:MODE]` with modes `saturate`, `wrap`, `drop`, `zero`) from lower-end precision reduction (`--round FIELD,PRECISION[:MODE]`).
  - `--round` strictly handles precision reduction and quantization: rounding floats to specified step intervals (`--round 0,0.01:round`) or quantizing integer fields to step intervals (`--round 0,10:round`).
  - Existing scripts using `--cut-maxint` continue to function identically with full backward compatibility.

### 1.3 Clarification and Architectural Evolution of Binary Probing (`--probe` & `--probe-units`) [COMPLETED]
- **Architecture & Pipeline Stages**:
  - **Raw Pre-Pipeline Prober (`--probe`)**: Pre-flight diagnostic scan (`src/main.rs`). Runs directly on raw `stdin` or target file before engine execution. Samples raw bytes and computes global Shannon entropy ($0..8$), byte distributions, $1..512$ byte strides, and ASCII string runs.
  - **Post-Processing Unit Stream Prober (`--probe-units` / `--probe-stream`)**: Runs inside the engine pipeline *after* input skips (`--input-skip-bits`, `--input-skip-units`), unit sizing (`--input-unit`), gaps (`--input-gap`), reversals, repeats, and pattern unpacking (`--input-pattern` / `--stream-pattern`).
  - **Per-Field Probing (`--probe-field=INDEX`)**: Targets a specific field of an unpacked container (e.g. payload field 1 in MPEG-TS) for entropy and distribution analysis.
- **Cryptographic Key Discovery (`--probe-keys` / `--probe-crypto-keys`) [COMPLETED]**:
  - Sliding-window localized entropy scanning across the unit stream.
  - Window sizes configurable via bits (128, 256, 512) or bytes (16B, 32B, 64B), default 256 bits (32 bytes).
  - Non-maximum suppression (NMS) isolates distinct peak candidates without reporting redundant 1-byte shifted windows.
  - Reports exact unit offset, bit offset, length, Shannon entropy, normalized entropy ratio, bit balance (% set bits), hex payload, and confidence classification.
  - Integrated into CLI text output, structured JSON (`--output-json`), and Model Context Protocol (`bdd_probe_units`).

### 1.4 Native Netlink & Kernel Telemetry Stream Ingestion (`--input-netlink`) [COMPLETED]
- **Motivation & Background**:
  The Linux kernel exposes ultra-fast binary telemetry and process lifecycle events over Netlink sockets:
  - `NETLINK_CONNECTOR` with `CN_IDX_PROC` (real-time `FORK`, `EXEC`, `EXIT`, `UID`/`GID`, `COMM` events without polling)
  - `NETLINK_GENERIC` / `TASKSTATS` (per-process and per-cgroup microsecond CPU, I/O delay, swap delay, and peak RSS accounting)
  - `NETLINK_INET_DIAG` (binary socket monitoring replacing `netstat`/`ss`)
  Currently, subscribing to Netlink multicast groups requires an external script/helper (such as `contrib/python/bdd_netlink_proc.py`) to open the Netlink socket, send multicast registration (`PROC_CN_MCAST_LISTEN`), and pipe the resulting packet stream into `bdd`.
- **Implemented Capabilities**:
  1. **Native Netlink Input Stream Source (`--input-netlink`)**:
     Direct Netlink socket binding to `AF_NETLINK` / `CN_IDX_PROC`, automatic multicast registration (`PROC_CN_MCAST_LISTEN`), and real-time kernel event streaming:
     ```bash
     bdd --input-netlink --output-json
     ```
  2. **Netlink Message Alignment & Framing**:
     Automatic stripping of 16-byte `nlmsghdr` and 20-byte Netlink connector headers (`cn_msg`), slicing the binary payload directly into target presets (`netlink-proc-event`, `proc-fork`, `proc-exec`, `proc-exit`).
  3. **Zero-Polling Real-Time Telemetry**:
     Native kernel process lifecycle event streaming entirely within `bdd` without any external Python or C wrappers.

---

## 2. Pattern Engine & Data Types

### 2.1 Void / Raw Bit Vector Pattern Type (`V` / `v`) [COMPLETED]
- **Current Behavior**: `V` (Big-Endian) and `v` (Little-Endian) represent opaque, unaligned bit vectors of arbitrary length (e.g. `13V`, `127V`, `1V`). Fields are unpacked and packed without numeric arithmetic coercion and formatted cleanly as bit sequences or `0b...` bitstrings in JSON/CSV.

---

## 3. CLI & Output Ergonomics

### 3.1 Visualizing Bit/Byte Reversal in Hex and Bit Sinks [COMPLETED]
- **Current Behavior**: `--output-reverse-unit` and `--output-reverse-bytes` apply symmetrically to `FileOutputStream`, `HexOutputStream`, `BitOutputStream`, and `IntegerOutputStream`, allowing direct visual inspection of reversed bits and bytes in the terminal.

### 3.2 Exhaustive Unit Cycle Shorthand (`--count=cycle` / `--count=full-range`) [COMPLETED]
- **Current Behavior**: `--count=cycle` and `--count=full-range` automatically calculate $2^{\text{unit\_size}}$ items from the resolved input unit or pattern, terminating after an exhaustive cycle. Exponentiation math expressions (e.g. `--count=2^16`, `2^8`) are also supported natively.

### 3.3 Heterogeneous Unit Sizes & Per-Stream Configuration for Multi-File Merge
- **Current State**:
  - The primary stream and merge streams can have different unit sizes from each other via `--input-unit` and `--merge-unit` (e.g. `--input-unit=12 --merge-unit=8` interleaves 12-bit units with 8-bit units).
  - However, when merging **multiple** secondary streams (via repeated `--merge-file` or `--merge-files`), **all merge streams must share the exact same unit size** (`--merge-unit`, default 8 bits), skip offsets, and gap parameters.
- **Problem & Motivation**:
  - Complex container multiplexing (e.g. interleaving a video elementary stream with audio frames, telemetry words, or subtitles) frequently requires different unit sizes per stream (e.g. 188-byte video packets interleaved with 16-bit audio samples and 1-bit sync flags).
- **Target Enhancements**:
  1. **Per-stream unit size list (`--merge-units`)**:
     Allow specifying comma-separated unit sizes corresponding by index to each merge file:
     ```bash
     bdd --input-file=video.raw --input-unit=188B \
         --merge-files=audio.raw,sync.raw --merge-units=16,1 \
         --output-file=muxed.bin
     ```
  2. **Structured inline parameter syntax on `--merge-file`**:
     Allow key-value attributes attached directly to each merge file argument:
     ```bash
     bdd --input-file=video.es --input-unit=188B \
         --merge-file="audio.es:unit=16:skip=32" \
         --merge-file="flags.bin:unit=1" \
         --output-file=muxed.bin
     ```
  3. **Per-stream pattern support (`--merge-patterns` / `:pattern=...`)**:
     Allow merge streams to unpack arbitrary bitfield patterns rather than only raw integers.

### 3.4 Future of Stream Arrows: Full Pipeline Unification (Sources, Slicers, Manipulators & Sinks)
- **Status**: Architecture Planned & Specified. RFC Published ([`docs/RFC-unified-stream-architecture-v2.md`](docs/RFC-unified-stream-architecture-v2.md), superseding [`docs/RFC-stream-arrows-unification.md`](docs/RFC-stream-arrows-unification.md)). Target: v0.6.0.
- **Vision**: Expressing entire end-to-end dataflows—from synthetic sources or Linux kernel sockets, through sub-byte bit slicers and in-flight manipulators, to structured JSON/hex sinks—as a single, human-readable pipeline string:
  ```bash
  bdd "zeros -> 8 -> xor(0xFF) -> json"
  bdd "netlink -> proc-event -> filter(what == 2) -> json"
  bdd "rand -> 256 -> hex" --count 10
  bdd "file('broadcast.ts') -> 188B[11:13] -> hex"
  bdd "4U4U -> 16U"
  ```
- **Unified Pipeline Model**: Eliminates redundant intermediate unit sizes (e.g. `8` and `16` in `"ones -> 4U4U -> 16U -> hex"`), introduces a strongly-typed pipeline transition model (`Bitstream` -> `BitUnit` -> `Tuple` -> `BitUnit` -> `Terminal`), and resolves all grammatical and dimensional ambiguities.

---

## 4. Low Priority & Exploratory Enhancements

### 4.1 Native Endianness Specifier
- **Priority**: Low. Modern platforms (x86_64, AArch64, RISC-V) are almost uniformly little-endian, and Rust already detects native endianness at compile time for internal optimizations.
- **Concept**: In addition to `U` (big-endian) and `u` (little-endian), a third variant for "native endianness" might be needed to decode host C structs, kernel trace buffers, or shared-memory IPC dumps portably across architectures.
- **Notation Challenge**: Finding a concise notation is difficult because casing (`U` vs `u`, `F` vs `f`) already distinguishes big-endian from little-endian. Potential options include a prefix modifier like `@` (matching Python `struct`), a symbol modifier, or a CLI flag (`--native-endian`).

---

## 5. Completed Features (Migrated from Legacy TODO)

The following items from the original 2010 `docs/TODO` scratchpad have been implemented and verified in modern `bdd` (v0.3.0):

- [x] **Output unit default (8 bits)**: Maintained original design where `--output-unit` defaults to 8 bits (preserving byte padding semantics and stream symmetry).
- [x] **Repeatable tuple manipulation pipeline**: Ordered transformation operators (`--round`, `--rearrange`, `--cut-maxint`, `--filter`, arithmetic/bitwise flags) executed sequentially.
- [x] **Unary `--abs` and `--sign` without dummy parameter**: Supports `--abs=FIELD` without requiring a trailing `,0`.
- [x] **Bare pattern types with implicit lengths**: Support for bare `x`, `u`/`U`, `b`/`B`, `f`/`F`, `d`/`D`, `e`/`E`, `h`/`H`, `y`/`Y`.
- [x] **Output pattern field discard**: `x` in output patterns skips fields from incoming tuples.
- [x] **Embedded sequence counters in patterns**: `K`/`k` in output patterns injects auto-incrementing counters.
- [x] **Container gap fast-seeking**: $O(1)$ filesystem seeking across periodic container gaps (`--input-gap` / `--input-raw-unit`).
- [x] **Drop partial EOF trailing bits**: `--drop-partial-eof` (aliases: `--drop-trailing-bits`, `--no-pad-eof`) discards incomplete trailing bits instead of synthetic zero padding.
- [x] **No trailing whitespace in hex/bit dumps**: Terminal sinks cleanly separate tokens without trailing line spaces.
- [x] **Multi-file round-robin merge**: Interleaving primary input with multiple secondary files via repeatable `--merge-file` and comma-separated `--merge-files`.
- [x] **Multi-gigabit Rust engine**: High-performance engine achieving 16 Gbps bit reversal and multi-gigabit streaming throughput.
- [x] **Warning deduplication & quiet mode on high-throughput streams**: Diagnostic warnings printed only on first occurrence during stream processing; deduplicated warning summary emitted at EOF. Added `-q` / `--quiet` flag to completely suppress non-fatal warnings and summaries for maximum pipeline throughput.
- [x] **Unified Stream I/O Mapping Syntax & Symmetrical Output Framing (Approach 1)**: Unified positional stream pattern argument (e.g. `8->3`, `123:8[2:4]+8 -> 5B:8[2:4]`) supporting both Form A (`raw[offset:unit]`) and Form B (`[pre:unit:post]`), flexible positional CLI syntax (`bdd [STREAM_PATTERN] [INPUT_PATTERN] [OUTPUT_PATTERN] [OPTIONS]`), and symmetrical output framing flags (`--output-raw-unit`, `--output-offset`, `--output-gap`, `--output-skip-bits`).
- [x] **Linux Kernel & Hardware Dissection Presets**: Added built-in format presets for kernel memory and hardware structures: `proc-pagemap` (64-bit page table entries: present, swapped, exclusive, dirty, pfn), `proc-auxv` (ELF 64-bit auxiliary vectors: AT_CLKTCK, AT_PAGESZ, AT_SECURE), `pci-config` (16-byte PCI device header: vendor, device, command, status, class), and `netlink-proc-event` (Netlink process connector event headers).
- [x] **Netlink Connector & Process Monitoring Suite**: Created `contrib/python/bdd_netlink_proc.py` for real-time, zero-polling Linux process lifecycle event streaming (FORK, EXEC, EXIT, UID/GID, COMM), `contrib/python/bdd_ps.py` for deep pagemap memory (USS / private exclusive memory) and signal mask inspection, and `contrib/python/bdd_top.py` for an interactive terminal monitor.
- [x] **Raw Memory & Cryptographic Key Streaming Tools**: Created `contrib/shell/stream_memory_keys.sh` and `contrib/python/stream_memory_keys.py` to stream system memory (physical RAM `/dev/mem`, kernel `/proc/kcore`, process virtual memory `/proc/[pid]/mem`, or standalone demo vectors) 256 bits at a time into `bdd --probe-keys=256` to locate high-entropy candidate cryptographic keys (AES-256, ChaCha20, Ed25519) with Shannon entropy, bit balance, and confidence scoring.
- [x] **Inline Unit Manipulations in Stream Patterns**: Multi-arrow stream notation supporting inline unit transformations between input and output specifications (e.g. `8 -> xor(0xFF) -> 8`, `8 -> add(10) -> mul(2) -> 8`).
- [x] **Visual Terminal Entropy Heatmap & Sparklines (`--probe-visual` / `--probe-map`)**: Terminal sparklines and 2D ANSI block heatmaps (` ▂▃▄▅▆▇█`) for rapid visual inspection of entropy gradients and compression boundaries.
- [x] **Magic Signature Scanning**: Automated detection of 25+ binary magic signatures (ELF, PNG, JPEG, PDF, ZIP, MPEG-TS, GZIP, WASM, PCAP, etc.) during binary probing.
- [x] **C & Rust Struct Code Generation (`--export-c` / `--export-rust`)**: Direct generation of packed C structs and Rust `#[repr(C, packed)]` definitions from arbitrary bitfield patterns or presets.
- [x] **Shell Auto-Completion (`--completions <SHELL>`)**: Generation of auto-completion scripts for `bash`, `zsh`, `fish`, `powershell`, and `elvish` via `clap_complete`.
- [x] **Model Context Protocol (MCP) Server Extensions**: Extended MCP stdio server with `bdd_transcode` (dynamic payload transcoding and bit-level transformations) and `bdd_generate` (synthetic vector generation).
- [x] **Native Linux Netlink Telemetry Ingestion (`--input-netlink`)**: Native kernel connector process event streaming directly inside `bdd`.


