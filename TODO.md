# bdd Roadmap & Planned Enhancements

This document consolidates high-value feature improvements, API additions, and architectural enhancements planned for `bdd`. It supersedes the legacy `docs/TODO` from the 2010 Python prototype.

---

## 1. High Priority Enhancements

### 1.1 Quoted Number Strings in `--input-tuples`
- **Current Behavior**: In `--input-tuples`, when an item is quoted (e.g. `"5"`), `Field::Bytes` attempts numeric parsing during BigUint conversion, converting it to numeric value `0x05`.
- **Target Behavior**: Quoted numbers must be strictly preserved as raw ASCII string bytes (`"5"` $\rightarrow$ ASCII `0x35` / `53`), while unquoted numbers (`5`) are parsed as numeric values (`0x05`).
- **Reference**: Resolves the note in `test/test.sh:151` (`echo -en "\"5\"" | ./bdd --input-tuples --output-hex` should output `35`).

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

### 3.3 Per-Stream Configuration for Multi-File Merge
- **Motivation**: Building on repeatable `--merge-file` and `--merge-files`, complex container muxing often requires different skip offsets or gap sizes per merge stream.
- **Specification**: Allow structured syntax in merge file arguments, e.g.:
  ```bash
  bdd --input-file=video.es --merge-file="audio.es:unit=16:skip=32" --merge-file="subtitles.es:unit=8"
  ```

---

## 4. Low Priority & Exploratory Enhancements

### 4.1 Native Endianness Specifier
- **Priority**: Low. Modern platforms (x86_64, AArch64, RISC-V) are almost uniformly little-endian, and Rust already detects native endianness at compile time for internal optimizations.
- **Concept**: In addition to `U` (big-endian) and `u` (little-endian), a third variant for "native endianness" might be needed to decode host C structs, kernel trace buffers, or shared-memory IPC dumps portably across architectures.
- **Notation Challenge**: Finding a concise notation is difficult because casing (`U` vs `u`, `F` vs `f`) already distinguishes big-endian from little-endian. Potential options include a prefix modifier like `@` (matching Python `struct`), a symbol modifier, or a CLI flag (`--native-endian`).

---

## 5. Completed Features (Migrated from Legacy TODO)

The following items from the original 2010 `docs/TODO` scratchpad have been implemented and verified in modern `bdd` (v0.3.0):

- [x] **Output unit default to input unit**: `--output-unit` defaults to input unit or pattern length instead of hardcoded 8.
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

