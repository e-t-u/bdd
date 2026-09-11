# bdd — Bit Data Dump

High-performance CLI tool and Rust library to unpack, manipulate, stream, merge, and pack arbitrary-width bitstreams.

Originally written in Python 2, `bdd` was re-architected in modern Rust for speed, memory safety, and 20-year long-term maintainability with zero external C library dependencies.

![Functionality Overview](diagram.png)

---

## Features

- **Arbitrary Bit-Widths**: Process bitfields of any length (not restricted to 8, 16, 32, or 64-bit boundaries).
- **Flexible Pattern Grammar**: Declarative input and output packing patterns supporting unsigned/signed integers, raw characters, floats, doubles, bit-reversed bytes, and padding constants.
- **Stream Generation & Merging**: Generate synthetic bit sequences (zeroes, ones, linear counters, PRNG noise) and interleave/merge multiple binary files.
- **Stream Transformations**: Rearrange field ordering, truncate bit ranges, clip integers to maximum bit bounds, execute bitwise XOR masks, and perform sign/magnitude conversions.
- **Multiple Output Formats**: Output raw binary bytes, hexadecimal representations, ASCII bit dumps (`0`/`1`), formatted integers, or CSV tuples.
- **Pure Safe Rust Core**: Clean library-first architecture (`src/lib.rs`) with typed error handling, no panics, and zero unsafe code.

---

## Installation

### Prerequisites

- Rust 1.70+ (`cargo`, `rustc`)

### Build & Install

```bash
# Build optimized release binary
make release

# Or install directly to ~/.cargo/bin
make install
```

To build manually with Cargo:

```bash
cargo build --release
cp target/release/bdd ~/.local/bin/bdd
```

---

## Pattern Grammar

Bit patterns specify the layout of fields within each input or output unit. A pattern consists of one or more field specifications, each written as `<bits><type>`:

| Specifier | Description | Allowed Bit Widths |
|---|---|---|
| `nU` | Unsigned integer | Any positive integer (e.g. `2U`, `12U`, `128U`) |
| `nS` | Signed integer (two's complement) | Any positive integer |
| `nC` | Character / byte string | Multiples of 8 bits (e.g. `8C`, `32C`) |
| `nc` | Character string with bit-reversed bytes | Multiples of 8 bits |
| `32F` | IEEE 754 single-precision float | Exactly 32 bits |
| `64D` | IEEE 754 double-precision float | Exactly 64 bits |
| `nZ` / `nz` | Constant zeroes (padding) | Output patterns only |
| `nO` / `no` | Constant ones (padding) | Output patterns only |

### Units vs Patterns

- `--input-unit=N` is a shorthand for `--input-pattern=NU`.
- `--output-unit=N` is a shorthand for `--output-pattern=NU`.

---

## Command Line Usage

```
Usage: bdd [OPTIONS] [FILE]

Arguments:
  [FILE]  Input file (defaults to standard input)

Options:
  -p, --input-pattern <PATTERN>    Bit pattern to unpack input
  -u, --input-unit <BITS>          Input unit size in bits (shorthand for <BITS>U)
  -t, --input-tuples               Read fields from comma-separated input lines
  -c, --input-counter              Generate sequential counter numbers
  -0, --input-zeros                Generate endless stream of zero bits
  -1, --input-ones                 Generate endless stream of one bits
  -r, --input-random               Generate pseudo-random bits
      --skip-bits <BITS>           Skip initial bits before processing [default: 0]
      --skip <UNITS>               Skip initial units before processing [default: 0]
      --count <COUNT>              Process at most COUNT units (0 = infinite) [default: 0]
      --reverse-input-bytes        Reverse bit order of each input byte
      --reverse-input-units        Reverse bit order of each input unit
      --output-pattern <PATTERN>   Bit pattern to pack output
      --output-unit <BITS>         Output unit size in bits (shorthand for <BITS>U)
  -x, --output-hex                 Output units as hexadecimal strings
  -b, --output-bit                 Output units as ASCII bit strings ('0' and '1')
  -i, --output-integer             Output unsigned integer representation per unit
  -T, --output-tuples              Output fields as comma-separated tuples
      --reverse-output-bytes       Reverse bit order of each output byte
      --reverse-output-units       Reverse bit order of each output unit
      --rearrange <FIELDS>         Reorder output fields by index (e.g. "1,0" or "-1,0")
      --cut-maxint <F,P>           Clip field F to fit in P bits (F:field index, P:bit width)
      --remove-right <F,P>         Remove P least-significant bits from field F
      --xor <F,P>                  Bitwise XOR field F with value P
      --abs <FIELD>                Convert signed field to absolute value
      --sign <FIELD>               Extract sign bit (0 = positive, 1 = negative)
      --merge-file <PATH>          Interleave stream from another file
  -h, --help                       Print help
  -V, --version                    Print version
```

---

## Examples

### 1. Simple Bit Inspection & Hex Dumps

Extract individual bits from standard input and display them:

```bash
echo -n "A" | bdd --input-unit=8 --output-bits
# Outputs: 01000001
```

Print hexadecimal values of 12-bit unpacked units:

```bash
bdd --input-unit=12 --output-hex < input.bin
```

### 2. Endianness & Byte Swapping

Swap every pair of adjacent bytes in a binary stream:

```bash
bdd --input-pattern=8U8U --rearrange=1,0 --output-pattern=8U8U < input.bin > swapped.bin
```

Reverse bit order within every byte:

```bash
bdd --reverse-input-bytes < input.bin > bits_reversed.bin
```

### 3. Bitfield Packing and Unpacking

Pack non-byte-aligned numbers into arbitrary bit containers:

```bash
# Combine 2-bit, 3-bit, and 3-bit fields into single 8-bit octets:
printf "1,2,3\n3,7,7\n" | bdd --input-tuples --output-pattern='2U3U3U' | od -t o1
# Outputs: 123 377
```

Unpack 10-bit ADC sensor data into comma-separated tuples:

```bash
bdd --input-pattern=10U --output-tuples < sensor.raw
```

### 4. Synthetic Stream Generation

Generate 16 sequential bytes formatted as hex:

```bash
bdd --input-counter --count=16 --output-hex
# Outputs: 00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f
```

Generate 1 megabyte of zero padding:

```bash
bdd --input-zeros --count=$((1024 * 1024)) --output-unit=8 > 1MB_zeros.bin
```

### 5. Stream Merging & Interleaving

Interleave a sequential index address with data bytes from `/etc/passwd`:

```bash
bdd --input-counter --count=16 --input-unit=12 --output-unit=12 --merge-file=/etc/passwd --output-hex
```

---

## Architecture & Codebase Design

The codebase is engineered defensively for decades-long maintainability:

```
src/
├── lib.rs          # Public library crate interface
├── main.rs         # Lean executable entry point & exit code translation
├── error.rs        # Strongly-typed BddError hierarchy
├── field.rs        # Arbitrary-precision Field enum & hardware-accelerated bit-reversals
├── pattern.rs      # Grammar parser, TupleUnpacker, and TuplePacker
├── stream.rs       # Stream generators (File, Counter, Zeros, Ones, Random, Tuples)
├── sink.rs         # Output writers (Binary, Hex, Bit, Integer, CSV Tuples)
├── manipulator.rs  # Field transformations (Rearrange, Xor, CutMaxint, Abs, Sign)
├── cli.rs          # Clap CLI definition & mutual-exclusion configuration validation
└── engine.rs       # End-to-end pipeline execution orchestrator
```

### Key Design Principles

1. **Strict Type Safety**: All errors flow through `BddError`. Functions return `Result<T, BddError>` instead of panicking or calling `std::process::exit`.
2. **Fast-Path Bit Reversal**: Sub-64-bit integer bit reversals execute via direct hardware `u64::reverse_bits()`, falling back to `BigUint` bit arithmetic only when necessary.
3. **Byte-Level String Integrity**: The `C` and `c` pattern types store raw bytes internally (`Field::Bytes`) rather than lossy UTF-8 conversions, guaranteeing bit-perfect roundtrips.
4. **Automated Verification**: Integrated test runner runs both native Rust unit tests and legacy golden-file integration tests.

---

## Development & Testing

Run the test suite:

```bash
make test
# or: cargo test --all-targets --all-features
```

Run code linter and formatting checks:

```bash
make lint
make check-fmt
```

---

## License

GPL-3.0-or-later. Original author: Esa Turtiainen.
