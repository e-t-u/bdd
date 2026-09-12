# bdd Roadmap & Planned Enhancements

This document consolidates high-value feature improvements, API additions, and architectural enhancements planned for `bdd`. It supersedes the legacy `docs/TODO` from the 2010 Python prototype.

---

## 1. High Priority Enhancements

### 1.1 Quoted Number Strings in `--input-tuples`
- **Current Behavior**: In `--input-tuples`, when an item is quoted (e.g. `"5"`), `Field::Bytes` attempts numeric parsing during BigUint conversion, converting it to numeric value `0x05`.
- **Target Behavior**: Quoted numbers must be strictly preserved as raw ASCII string bytes (`"5"` $\rightarrow$ ASCII `0x35` / `53`), while unquoted numbers (`5`) are parsed as numeric values (`0x05`).
- **Reference**: Resolves the note in `test/test.sh:151` (`echo -en "\"5\"" | ./bdd --input-tuples --output-hex` should output `35`).

### 1.2 Overhaul of Rounding, Range Clamping, and Floating-Point Handling
- **Problem Statement**:
  - The current `--round` / `--cut-maxint` manipulator conflates two fundamentally distinct operations:
    1. **Upper-End Magnitude / Range Overflow (MSB Cutting)**: When a larger value must fit into a smaller unit (e.g. value 300 into an 8-bit container max 255), the *most significant bits* exceed the range. This is **clamping, saturation, wrapping, or high-bit clipping**—it is *not* rounding.
    2. **Lower-End Precision Reduction (LSB Rounding)**: When reducing precision, fractional digits, or low-order bits, bits are removed from the *least significant part*. This is **true rounding / quantization**.
- **Options Analysis in `--round`**:
  - **Options that do NOT make sense in rounding (only in truncation / clamping / filtering)**:
    - `saturate` / `clamp`: Magnitude capping to `[-LIMIT, LIMIT]`. This is range saturation, not rounding.
    - `wrap` / `modulo`: High-bit truncation discarding overflow MSBs modulo $(LIMIT + 1)$. This is integer overflow wrapping, not rounding.
    - `zero` / `reset`: Out-of-bounds reset policy setting out-of-range values to 0. Not rounding.
    - `drop` / `filter` / `checked`: Discards the tuple entirely on range overflow. This is record filtering, not rounding.
    - The `LIMIT` parameter itself: Passing a maximum magnitude threshold (e.g. `--round 0,255`) is a bounding limit, not a rounding step or precision scale.
  - **Options that belong to true rounding (precision reduction on fractional parts / LSBs)**:
    - `round` / `half_up`: Round to nearest neighbor, ties away from zero.
    - `round_ties_even` / `bankers`: Round to nearest neighbor, ties to nearest even digit (IEEE 754 default).
    - `floor`: Round toward $-\infty$.
    - `ceil`: Round toward $+\infty$.
    - `trunc`: Round toward zero (fractional truncation).
  - **Implementation Flaws in `RoundManipulator`**:
    - For integer fields with a `LIMIT`, specifying `round`, `floor`, `ceil`, `trunc`, or `round_ties_even` silently degenerates to `saturate` (clamping).
    - For integer fields without a `LIMIT`, `--round 0,round` is an inert no-op because integers lack fractional parts and `bdd` lacks integer step quantization (e.g. rounding to nearest 10 or nearest $2^k$).
    - For floating-point fields, `saturate` and `wrap` bypass rounding and merely clamp/wrap the float magnitude.
- **Target Architecture & Next Steps**:
  - Decouple upper-end range overflow handling (clamping / saturation / wrapping via `--cut-maxint` or `--clamp`) from lower-end precision reduction (rounding / quantization via `--round`).
  - Reserve `--round` strictly for precision reduction: rounding floats to integers or rounding floats/integers to a specified step/precision (`--round FIELD,PRECISION[,MODE]`).
  - Re-examine floating-point codecs and pipelines to avoid unintended `f64` conversions when bit-exactness is required, and ensure rounding modes applied to float mantissas and integers are mathematically precise and consistent.

### 1.3 Clarification and Architectural Evolution of Binary Probing (`--probe`)
- **Current Architecture & Pipeline Stage**:
  - **Where Probe Executes Today**: `--probe` is currently implemented as an **out-of-band pre-flight diagnostic scan** (`src/main.rs:83-113`). It runs directly on the raw input stream or file (`stdin` or target file) **before** the engine pipeline starts, and terminates immediately after printing its report without running the pipeline.
  - **What It Currently Bypasses**: It does not pass through stream patterns (`--stream-pattern`), container slicing (`8[2:6]`), unit extraction (`--input-unit`), or tuple manipulation (`--rearrange`, `--round`). It samples up to 1 MB of raw input bytes and calculates:
    1. Overall Shannon entropy ($0.0 - 8.0$).
    2. Byte class distributions (null bytes, printable ASCII, high bytes).
    3. Periodic autocorrelation strides ($1..512$ bytes) to identify container packet lengths (e.g. 188-byte MPEG-TS).
    4. Printable ASCII string runs.
  - **Documentation Deficit**: The user documentation (`README.md`, `bdd.1`) needs a much clearer explanation of what `--probe` does, how each metric should be interpreted, and where in the data lifecycle it operates.
- **Proposed Feature: "Find Crypto Keys / High-Entropy Regions"**:
  - **Motivation**: In binary reverse engineering, firmware auditing, and forensic analysis, a single global entropy score is insufficient—an uncompressed firmware image or memory dump might have moderate overall entropy (~4.5), but contain embedded 256-bit AES keys, RSA private keys, or encrypted payload blocks with maximal entropy ($H \approx 8.0$).
  - **Target Feature**: Sliding-window localized entropy scanning (`--probe-entropy-scan` or `--probe-keys`):
    - Slides across the stream using configurable window sizes (e.g. 16, 32, 64, 256 bytes).
    - Detects localized entropy spikes ($H > 7.8$) to pinpoint candidate encryption keys, IVs, ciphertext blobs, or compressed segments.
    - Outputs candidate offset locations, bit ranges, length, and local entropy score in human-readable output and structured JSON (`--output-json`).
- **Proposed Architectural Evolution: Probing Inside the Processing Pipeline**:
  - **Post-Pattern / Per-Field Probing (`--probe-field=F` / `--probe-tuple`)**:
    - Allow probing *after* the stream unpacker / container pattern has sliced the stream.
    - *Use Case*: In a multiplexed container (e.g. MPEG-TS, telemetry frames, or custom protocol headers), measure the entropy and distribution of individual fields across packets (e.g. check if field 1 / payload is encrypted while field 0 / header remains structured).
  - **Unit-Level Probing**:
    - Allow computing entropy and periodicity across unpacked units rather than only raw bytes.

---

## 2. Pattern Engine & Data Types

### 2.1 Void / Raw Bit Vector Pattern Type (`V` / `v`)
- **Motivation**: Non-byte-aligned opaque payloads (e.g. a 13-bit video payload or 127-bit cryptographic block) currently must use `U` (which treats them as arithmetic integers) because `B` requires byte multiples ($N \times 8$).
- **Specification**:
  - `V` (Big-Endian) and `v` (Little-Endian) represent opaque, unaligned bit vectors of arbitrary length (e.g., `13V`, `127V`).
  - Pass through pipelines without numeric arithmetic coercion.
  - Formatted cleanly as raw bit sequences in binary streams or bit-accurate hex in JSON/CSV.

---

## 3. CLI & Output Ergonomics

### 3.1 Visualizing Bit/Byte Reversal in Hex and Bit Sinks
- **Current Behavior**: `--output-reverse-unit` and `--output-reverse-bytes` only apply to `FileOutputStream` (binary file output). `--output-hex` and `--output-bits` display the unreversed BigUint value.
- **Target Behavior**: Allow `--output-hex` and `--output-bits` to optionally reflect the reversed unit/bytes (or provide `--output-inspect-reversed`), allowing direct visual inspection of reversed bits in the terminal.

### 3.2 Exhaustive Unit Cycle Shorthand (`--count=cycle` / `--count=full-range`)
- **Motivation**: `--input-counter` generates an infinite sequence by default. For testing, generating a complete cycle of all values in a unit ($2^{\text{unit}}$ items) is a common requirement.
- **Specification**: Support `--count=cycle` or `--count=full-range` (or math expressions like `2^16`), which automatically terminates the stream after counting exactly $2^{\text{unit\_size}}$ values.

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
- [ ] **Inline Unit Manipulations in Stream Patterns**: Extend the stream arrow notation to support inline unit transformations between input and output specifications (e.g. `<in> -> <manip> -> <out>`).


