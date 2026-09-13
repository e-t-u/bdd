# bdd-small-floats

Specialized sub-byte, AI, and GPU floating-point codecs extracted from [`bdd`](https://github.com/e-t-u/bdd).

Provides standalone encoding and decoding routines between `f64` and compact floating-point representations:
- **FP16** (`16H` / `16h`): IEEE 754 half-precision float (16-bit).
- **BF16** (`16Y` / `16y`): Google Brain bfloat16 float (16-bit).
- **FP8 E4M3** (`8E` / `8e`): OCP FP8 E4M3FN format (8-bit, 1 sign, 4 exp, 3 mantissa).
- **FP8 E5M2** (`8Q` / `8q`): OCP FP8 E5M2 format (8-bit, 1 sign, 5 exp, 2 mantissa).
- **FP6 E3M2** (`6E` / `6e`): OCP FP6 E3M2 format (6-bit, 1 sign, 3 exp, 2 mantissa).
- **FP4 E2M1** (`4E` / `4e`): NVIDIA Blackwell / OCP FP4 E2M1 microscaling format (4-bit, 1 sign, 2 exp, 1 mantissa).

## Features

- **Zero dependencies**: Pure Rust, no external crates required.
- **Fast and lightweight**: Direct bitwise bitfield manipulation, clamp and rounding.
- Extensively tested against known IEEE, OCP, and NVIDIA specifications.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
bdd-small-floats = { path = "crates/bdd-small-floats" }
```

### Example

```rust
use bdd_small_floats::{decode_f16, encode_f16, decode_fp8_e4m3, encode_fp8_e4m3};

let bits = encode_f16(1.5);
assert_eq!(decode_f16(bits), 1.5);

let fp8_bits = encode_fp8_e4m3(1.0);
assert_eq!(decode_fp8_e4m3(fp8_bits), 1.0);
```

## License

GPL-3.0
