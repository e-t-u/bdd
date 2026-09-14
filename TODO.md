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

### 2.2 Direct Memory-Mapped File I/O (`mmap` Feature) [COMPLETED]
- **Current Behavior**: Implemented zero-copy memory-mapped file access via `memmap2` behind an optional Cargo feature (`mmap`, enabled by default). Regular input and merge files are automatically memory mapped with kernel sequential readahead (`MADV_SEQUENTIAL`), providing zero-copy buffer slicing and instant $O(1)$ seeking across container gaps without `read()` syscall overhead. Seamlessly falls back to standard buffered streams on stdin, pipes, FIFOs, and 0-byte files. Can be toggled at runtime via `--mmap` and `--no-mmap` (and `--merge-mmap` / `--merge-no-mmap`), and can be disabled at compile time via `--no-default-features` for minimal or embedded platforms.

### 2.3 Small-Integer Fast Path (`BitValue`) [COMPLETED]
- **Motivation & Problem**: In earlier versions, all bitstream units were converted to heap-allocated `BigUint` instances regardless of size (even for 1-bit, 3-bit, 8-bit, or 32-bit units). This caused substantial memory allocator churn in the inner streaming loop, limiting 1-bit streams to ~36 Mbps and 8-bit streams to ~270 Mbps.
- **Current Architecture**:
  - Introduced `BitValue` (`Inline(u64)` for widths $\le 64$ bits, `Big(BigUint)` for arbitrary bignum widths $> 64$ bits) with zero heap allocations for $>95\%$ of typical workloads.
  - Implemented fast-path register operations across streams (`next_bit_value`), sinks (`write_bit_value`, `write_u64`), pattern unpackers (`unpack_u64`, `unpack_bit_value`), and packers (`pack_u64`, `pack_bit_value`).
  - Added a passthrough streaming fast-path in `engine.rs` bypassing field vector allocation when no manipulators or unpackers are active.
- **Benchmark Improvements**:
  - **1-bit single-bit resolution stream**: 36.33 Mbps $\rightarrow$ **231.31 Mbps** (**6.37x faster, +537%**)
  - **Synthetic 8-bit stream generation**: 271.74 Mbps $\rightarrow$ **1.16 Gbps** (**4.26x faster, +326%**)
  - **Unaligned 3-bit unpack to 8-bit**: 49.32 Mbps $\rightarrow$ **203.11 Mbps** (**4.12x faster, +312%**)
  - **Tuple pipeline (`2U3U3U` $\rightarrow$ rearrange)**: 31.10 Mbps $\rightarrow$ **90.74 Mbps** (**2.92x faster, +192%**)

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
- [x] **Decoupled Netlink Process Ingestion to `contrib/` (`contrib/rust/bdd-netlink`)**: Decoupled kernel Netlink connector stream ingestion from core `bdd` into an ultra-fast, zero-dependency standalone Rust utility (`contrib/rust/bdd-netlink`, compiled to `contrib/bdd-netlink`). Preserved `netlink-proc-event` in default format presets (`presets.json`), enabling clean Unix piping (`sudo ./contrib/bdd-netlink | bdd --preset netlink-proc-event --output-json`) while eliminating unsafe OS-specific socket code from the core binary.
- [x] **Decoupled Web UI Server (`web/`)**: Decoupled the interactive HTTP Web UI from the core `bdd` Rust crate into a dedicated, self-contained `web/` directory. Core crate binary has zero web server overhead. Decoupled server communicates exclusively via the `bdd` CLI through subprocess pipes (`web/server.py` with pure Python stdlib and zero external dependencies, and `web/Cargo.toml` / `web/src/main.rs` with minimal serde dependencies).
- [x] **Decoupled Presets File & Remote Preset Downloading (`presets.json` / `--download-presets` / `--presets-file`)**: Extracted all protocol and float presets into an external `presets.json` file. Automatically downloads the default presets file from GitHub on first run (with fallback to embedded defaults when offline). Added `--download-presets [URL]` to fetch and install custom or updated preset files, and `--presets-file <PATH>` / `BDD_PRESETS_FILE` for alternate preset collections.
- [x] **Standalone Small Floating-Point Crate (`bdd-small-floats` & `small-floats` feature)**: Extracted specialized sub-byte, AI, and GPU float codecs (FP16, BF16, OCP FP8 E4M3FN, OCP FP8 E5M2, OCP FP6 E3M2, NVIDIA Blackwell FP4 E2M1) into a dedicated, zero-dependency standalone crate (`crates/bdd-small-floats`). Verified that all float codec references in `bdd` (`src/pattern.rs`, `src/ffi.rs`, `src/preset.rs`, `src/cli.rs`) are strictly gated behind the `small-floats` Rust feature flag, enabling a minimal zero-cost binary when building with `--no-default-features`.
- [x] **Decoupled Model Context Protocol Server (`crates/bdd-mcp`)**: Extracted MCP server from `src/mcp.rs` into a standalone companion crate and binary `crates/bdd-mcp` in `workspace.members`. Core `bdd --mcp` seamlessly delegates to `bdd-mcp` (or provides a helpful launch notice), isolating rapid LLM/agent protocol evolution from the core bitstream library.
- [x] **Unified Pattern & Stream Framing AST (`FramedPattern` in `src/pattern.rs`)**: Unified the previously disjoint ASTs in `stream_pattern.rs` and `pattern.rs` into a single, cohesive `FramedPattern` AST where container framing (`188B[...]`, `[pre:unit:post]`, `skip:raw[offset:unit]+gap`) wraps field definitions (`8U,4S`, `sync:11u,pid:13u`). Replaced nearly 200 lines of duplicate parser code in `stream_pattern.rs` with `FramedPattern::parse`.
- [x] **Decoupled Struct Transpilers to `contrib/` (`json_to_c.py` & `json_to_rust.py`)**: Provided standalone, template-driven tools in `contrib/python/` (`json_to_c.py` and `json_to_rust.py`) translating `--explain-pattern <PAT> --output-json` to packed C and Rust structs.
- [x] **In-Place Container Overwrite (`overwrite`) & Set Manipulator (`set(...)`)**: Implemented non-destructive container patching preserving skips, gaps, and surrounding headers, plus arbitrary bitfield value assignment (`set(0x28)`, `set(0, 0xF)`).
- [x] **Container & Unit Terminology Unification**: Standardized container/unit definitions across code and documentation, removing legacy positional parameters 2 and 3.
- [x] **CLI Architecture Consolidation**: Removed `--serve` stub flag, consolidated redundant seeking and memory-mapping flags (`--no-seek`, `--no-mmap`, `--mmap`, `--input-use-seek`) with backward-compatible aliases.
- [x] **Offline-Safe Preset Discovery**: Removed online `--download-presets` and subprocess invocations (`curl`, `wget`, `python3`) from `preset.rs`. Missing presets are seeded synchronously from embedded defaults; custom paths are loaded via `--presets-file` or `BDD_PRESETS_PATH`.
- [x] **Cargo Feature Decoupling for Prober (`probe` feature)**: Decoupled `src/probe.rs` behind `#[cfg(feature = "probe")]`. Enabled by default, but allows building ultra-minimal bitstream slicers with zero probe dependencies via `--no-default-features --features mmap`.
- [x] **Deprecated Struct Code Generation Flags**: Deprecated `--export-c` and `--export-rust` with warnings in favor of `contrib/python/json_to_c.py` and `json_to_rust.py`.
- [x] **Foundational Mathematical & Information-Theoretic Engine (`src/analysis/`)**: Extracted zero-dependency Shannon entropy, normalized entropy, bit balance, and Hamming weight into `src/analysis/math.rs`. Created `Accumulator` trait and implementations (`Count`, `Sum`, `Min`, `Max`, `Mean`, `Entropy`, `BitBalance`, `Variance`, `Distinct`) and `TupleCollector` profiler (`src/analysis/profile.rs`). Extended `Field` with direct analysis methods (`.shannon_entropy()`, `.bit_balance()`, `.hamming_weight()`).

---

## 6. Active Roadmap & Unimplemented Items

### 6.1 Rolling / Sliding-Window Stream Accumulators in Manipulators (Option 2)
- **Concept**: Stateful sliding-window accumulators operating inside the stream manipulation pipeline. Emits 1 output tuple per incoming tuple in $O(1)$ time and $O(W)$ bounded memory without buffering the entire stream.
- **Target Syntax**:
  - `12u -> moving_avg(window=16) -> hex`: Smoothing sensor ADC readings over a 16-sample window.
  - `8u -> rolling_entropy(window=64) -> filter(entropy > 7.8) -> ...`: Continuous entropy boundary detector.
- **Architecture**: `SlidingWindow<A: Accumulator>` ring-buffer wrapper struct in `src/analysis/accumulator.rs` exposed via `TupleManipulator` in `src/manipulator.rs`.

### 6.2 Keyed Stream Grouping & Terminal Summary Sinks (Option 3)
- **Keyed Grouping (`group_by`)**:
  - Partitions incoming tuples by a key field and maintains per-group metric accumulators.
  - Example: `bdd broadcast.ts "[ sync:8u, pid:13u, payload:184*8u ] -> group_by(pid, count(), sum(payload), avg(entropy(payload))) -> csv"`
- **Terminal Profiler Sink (`-> stats` / `-> profile`)**:
  - Ingests all tuples through `TupleCollector` and emits an aligned ASCII table or JSON profile at EOF without printing raw tuples.
  - Example: `bdd traffic.bin "ipv4-header -> stats"`

### 6.3 In-Pipeline Field Analysis Functions
- **Projection Expressions**:
  - Support analysis metrics directly in `{...}` projections: `[ sync:8u, pid:13u, payload:184*8u ] -> { pid, entropy(payload) } -> json`.
- **Predicate Filtering**:
  - Support analysis metrics in filter predicates: `filter(entropy(payload) > 7.8)` (extracting only encrypted packets).

### 6.4 Heterogeneous Unit Sizes for Multi-File Merge
- Allow specifying comma-separated unit sizes or per-stream attributes corresponding to each merge file:
  ```bash
  bdd --input-file=video.raw --input-unit=188B --merge-files=audio.raw,sync.raw --merge-units=16,1
  ```

### 6.5 Native Endianness Specifier (`@` / `--native-endian`)
- Host-architecture native endianness modifier for portable binary struct decoding across differing CPU architectures.

