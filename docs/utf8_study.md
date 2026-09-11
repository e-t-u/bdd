# UTF-8 Input Stream Processing in `bdd`: Architectural Study & Hazards

## 1. Executive Summary

`bdd` is designed to interpret, manipulate, and generate arbitrary-precision bit streams. When handling character and text streams—specifically UTF-8—several distinct mechanical challenges arise due to the fundamental differences between **fixed-width integer bitfields** and **variable-width self-synchronizing byte sequences**.

This study analyzes how `bdd` interacts with UTF-8 input streams, details the hazards of bit-level slicing and endianness reversals, and documents architectural safeguards.

---

## 2. UTF-8 Structural Anatomy

UTF-8 encodes Unicode scalar values ($0\text{x}0000$ to $0\text{x}10\text{FFFF}$) into variable-length sequences of 1 to 4 bytes:

| Byte Count | Scalar Range (Hex) | Byte 1 (Lead) | Byte 2 | Byte 3 | Byte 4 | Payload Bits |
|:---|:---|:---|:---|:---|:---|:---|
| 1 byte (ASCII) | `0000` – `007F` | `0xxxxxxx` | — | — | — | 7 bits |
| 2 bytes (Latin, Finnish ä/ö) | `0080` – `07FF` | `110xxxxx` | `10xxxxxx` | — | — | 11 bits |
| 3 bytes (Euro €, CJK) | `0800` – `FFFF` | `1110xxxx` | `10xxxxxx` | `10xxxxxx` | — | 16 bits |
| 4 bytes (Emoji, Symbols) | `10000` – `10FFFF` | `11110xxx` | `10xxxxxx` | `10xxxxxx` | `10xxxxxx` | 21 bits |

### Key Structural Constraints:
1. **Prefix markers are position-dependent**: The leading byte uniquely determines sequence length via its high-order bit pattern (`0`, `110`, `1110`, `11110`).
2. **Continuation byte headers**: Every subsequent byte in a multi-byte sequence must begin with `10xxxxxx` (`0x80`..`0xBF`).
3. **Byte-order independence**: UTF-8 is defined strictly as an ordered sequence of 8-bit octets. It has no endianness.

---

## 3. Bitstream Hazards in `bdd`

When processing UTF-8 data through `bdd`, three classes of transformation hazards exist:

### 3.1. Non-Byte-Aligned Bit Shifting
* **Mechanism**: When `--input-skip-bits N` (where $N \not\equiv 0 \pmod 8$) or non-multiple-of-8 patterns (e.g. `3U5U`) are applied.
* **Hazard**: Slicing across bit boundaries shifts the prefix patterns (`110...`, `10...`). A shifted `10xxxxxx` byte becomes an invalid prefix, causing downstream decoders to report encoding errors.
* **Preservation Rule**: If UTF-8 integrity is required, bit offsets must remain strictly aligned to 8-bit boundaries.

### 3.2. Endianness & Bit Reversal Inversion
* **Mechanism**: Options `--input-little-endian`, `--input-reverse-bytes`, or `--input-reverse-unit`.
* **Hazard**: 
  - `--input-little-endian` reverses byte order within multi-byte units. If reading a 16-bit unit spanning a Finnish `ä` (`0xC3 0xA4`), little-endian interpretation swaps the octets to `0xA4 0xC3`. The sequence now begins with a continuation byte `0xA4`, corrupting the stream.
  - `--input-reverse-bytes` flips the internal bits of every octet ($b_0 \leftrightarrow b_7$). The lead byte prefix `110xxxxx` becomes `xxxxx011`, destroying the framing markers.
* **Preservation Rule**: UTF-8 text must never be processed with byte or bit reversal unless deliberately obfuscating or analyzing raw hardware scrambling.

### 3.3. Unit Truncation & Overwrite
* **Mechanism**: Output patterns specifying fewer bits than required for the character (e.g., writing a 2-byte character into an `8C` field).
* **Hazard**: High-order bits or trailing continuation bytes are truncated, producing orphaned lead bytes (`0xC3` without `0xA4`).

---

## 4. Architectural Implementation in `bdd`

### 4.1. Lossless Raw Byte Storage (`Field::Bytes`)
In `src/field.rs`:
```rust
pub enum Field {
    UInt(BigUint),
    Int(BigInt),
    Float(f64),
    Bytes(Vec<u8>),
}
```
* The `C`/`c` (character/byte array) pattern unpacker stores bytes directly into `Field::Bytes(Vec<u8>)`.
* When formatted as text, `Field::Bytes` uses lossy UTF-8 formatting (`String::from_utf8_lossy`), replacing broken sequences with Unicode replacement characters (``) rather than crashing.

### 4.2. Safe Line Ingestion in `TupleDirectInput`
In `src/stream.rs`:
* Ingestion uses `BufRead::read_until(b'\n', &mut raw_bytes)`.
* Instead of strict UTF-8 decoding (`read_line`), which aborts with an I/O error on invalid bytes, `bdd` reads arbitrary byte lines losslessly and parses fields using robust CSV handling.

---

## 5. Future Evolution: Native Unicode Scalar Stream (`W`/`w`)

To natively manipulate UTF-8 text streams without manual byte reconstruction, `bdd` can introduce the `W` (Wide Unicode Scalar) pattern:

* **Unpack `W`**: Reads variable-length UTF-8 bytes (1 to 4 bytes) from the stream and unpacks them into a single 21-bit integer scalar value (`0x0000`..`0x10FFFF`).
  - Example: Finnish `ä` (`0xC3 0xA4`) unpacks directly to integer `228` (`0xE4`).
* **Pack `W`**: Takes a 21-bit integer code point and encodes it back into the standard UTF-8 1-to-4 byte representation.
* **Manipulate `W`**: Allows character manipulation using arithmetic flags:
  - Rot13 / Caesar cipher: `--add 0,13`
  - Case conversion: `--sub 0,32`
  - Codepoint filtering: `--filter 0,>=,0x0400` (filtering Cyrillic script)
