use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{Num, One, ToPrimitive, Zero};
use std::fmt;

/// Parses an integer string supporting decimal, hex (`0x`/`0X`), octal (`0o`/`0O`), and binary (`0b`/`0B`) prefixes.
pub fn parse_radix_bigint(s: &str) -> Option<BigInt> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (sign, rest) = if let Some(stripped) = trimmed.strip_prefix('-') {
        (-1, stripped)
    } else if let Some(stripped) = trimmed.strip_prefix('+') {
        (1, stripped)
    } else {
        (1, trimmed)
    };

    let val = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        BigInt::from_str_radix(hex, 16).ok()
    } else if let Some(bin) = rest.strip_prefix("0b").or_else(|| rest.strip_prefix("0B")) {
        BigInt::from_str_radix(bin, 2).ok()
    } else if let Some(oct) = rest.strip_prefix("0o").or_else(|| rest.strip_prefix("0O")) {
        BigInt::from_str_radix(oct, 8).ok()
    } else {
        BigInt::from_str_radix(rest, 10).ok()
    };

    val.map(|v| if sign == -1 { -v } else { v })
}

/// Represents a single field value within a tuple.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    UInt(BigUint),
    Int(BigInt),
    Float(f64),
    Bytes(Vec<u8>),
    Bits(BigUint, usize),
}

impl Field {
    /// Convert this field to an unsigned big integer.
    pub fn as_biguint(&self) -> BigUint {
        match self {
            Field::UInt(u) => u.clone(),
            Field::Bits(u, _) => u.clone(),
            Field::Int(i) => {
                if i >= &BigInt::zero() {
                    i.to_biguint().unwrap_or_else(BigUint::zero)
                } else {
                    BigUint::zero()
                }
            }
            Field::Float(f) => {
                if *f >= 0.0 {
                    BigUint::from(*f as u64)
                } else {
                    BigUint::zero()
                }
            }
            Field::Bytes(bytes) => {
                let mut val = BigUint::zero();
                for &b in bytes {
                    val = (val << 8) | BigUint::from(b);
                }
                val
            }
        }
    }

    /// Convert this field to a signed big integer.
    pub fn as_bigint(&self) -> BigInt {
        match self {
            Field::UInt(u) => BigInt::from_biguint(Sign::Plus, u.clone()),
            Field::Bits(u, _) => BigInt::from_biguint(Sign::Plus, u.clone()),
            Field::Int(i) => i.clone(),
            Field::Float(f) => BigInt::from(*f as i64),
            Field::Bytes(_) => BigInt::from_biguint(Sign::Plus, self.as_biguint()),
        }
    }

    /// Convert this field to a 64-bit float.
    pub fn as_f64(&self) -> f64 {
        match self {
            Field::UInt(u) => u.to_f64().unwrap_or(0.0),
            Field::Bits(u, _) => u.to_f64().unwrap_or(0.0),
            Field::Int(i) => i.to_f64().unwrap_or(0.0),
            Field::Float(f) => *f,
            Field::Bytes(bytes) => {
                if let Ok(s) = std::str::from_utf8(bytes) {
                    s.parse::<f64>().unwrap_or(0.0)
                } else {
                    0.0
                }
            }
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Field::UInt(u) => write!(f, "{}", u),
            Field::Int(i) => write!(f, "{}", i),
            Field::Float(fl) => write!(f, "{}", fl),
            Field::Bytes(bytes) => write!(f, "{}", String::from_utf8_lossy(bytes)),
            Field::Bits(u, bits) => {
                let b_str = format!("{:b}", u);
                let filler = "0".repeat(bits.saturating_sub(b_str.len()));
                write!(f, "{}{}", filler, b_str)
            }
        }
    }
}

/// Reverses the lowest `bits` bits of `val`, keeping any higher bits in place.
pub fn reverse_bits(val: &BigUint, bits: usize) -> BigUint {
    if bits == 0 {
        return val.clone();
    }

    // Fast-path: native register bit-reversal for units <= 64 bits that fit in u64
    if bits <= 64 {
        if let Some(v) = val.to_u64() {
            let high = if bits == 64 { 0 } else { v >> bits };
            let mask = if bits == 64 {
                !0u64
            } else {
                (1u64 << bits) - 1
            };
            let low = v & mask;
            let rev = low.reverse_bits() >> (64 - bits);
            let res = if bits == 64 {
                rev
            } else {
                (high << bits) | rev
            };
            return BigUint::from(res);
        }
    }

    // Arbitrary-precision fallback
    let mut out = val >> bits;
    let mut v = val.clone();
    let one = BigUint::one();
    for _ in 0..bits {
        out <<= 1;
        if (&v & &one) != BigUint::zero() {
            out += &one;
        }
        v >>= 1;
    }
    out
}

/// Reverses the bits within each byte of a unit of the given bit width.
pub fn reverse_unit_bytes(val: &BigUint, bits: usize) -> BigUint {
    if bits == 0 {
        return BigUint::zero();
    }
    let mut out = BigUint::zero();
    let mut rem_bits = bits;
    while rem_bits > 0 {
        let chunk_bits = rem_bits.min(8);
        let shift = rem_bits - chunk_bits;
        let mask = if chunk_bits > 0 {
            (BigUint::one() << chunk_bits) - 1u32
        } else {
            BigUint::zero()
        };
        let chunk = ((val >> shift) & mask).to_u8().unwrap_or(0);
        let rev_chunk = chunk.reverse_bits() >> (8 - chunk_bits);
        out = (out << chunk_bits) | BigUint::from(rev_chunk);
        rem_bits -= chunk_bits;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverse_bits_u64() {
        assert_eq!(reverse_bits(&BigUint::from(1u32), 8), BigUint::from(128u32));
        assert_eq!(
            reverse_bits(&BigUint::from(0xAA01u32), 8),
            BigUint::from(0xAA80u32)
        );
        assert_eq!(
            reverse_bits(&BigUint::from(0x55u32), 8),
            BigUint::from(0xAAu32)
        );
        assert_eq!(reverse_bits(&BigUint::from(0u32), 16), BigUint::zero());
        assert_eq!(
            reverse_bits(&BigUint::from(0x0123456789ABCDEFu64), 64),
            BigUint::from(0x0123456789ABCDEFu64.reverse_bits())
        );
    }

    #[test]
    fn test_reverse_bits_large() {
        let big = (BigUint::one() << 100) | BigUint::one();
        let rev = reverse_bits(&big, 101);
        // Bit 0 was 1 (now bit 100), bit 100 was 1 (now bit 0)
        assert_eq!(rev, big);

        // 256 bits
        let val_256 = (BigUint::one() << 255) | (BigUint::one() << 100) | BigUint::one();
        let rev_256 = reverse_bits(&val_256, 256);
        let rev_rev_256 = reverse_bits(&rev_256, 256);
        assert_eq!(rev_rev_256, val_256);
        assert_eq!((&rev_256 >> 255) & BigUint::one(), BigUint::one());
        assert_eq!(&rev_256 & BigUint::one(), BigUint::one());

        // 1024 bits
        let val_1024 = (BigUint::one() << 1023) | (BigUint::one() << 512);
        let rev_1024 = reverse_bits(&val_1024, 1024);
        assert_eq!(reverse_bits(&rev_1024, 1024), val_1024);
        assert_eq!(&rev_1024 & BigUint::one(), BigUint::one());
        assert_eq!((&rev_1024 >> 511) & BigUint::one(), BigUint::one());

        // 4096 bits
        let val_4096 = (BigUint::one() << 4095) | BigUint::from(0xDEADBEEFu64);
        let rev_4096 = reverse_bits(&val_4096, 4096);
        assert_eq!(reverse_bits(&rev_4096, 4096), val_4096);
    }

    #[test]
    fn test_field_conversions() {
        let f_u = Field::UInt(BigUint::from(42u32));
        assert_eq!(f_u.as_bigint(), BigInt::from(42));
        assert_eq!(f_u.as_f64(), 42.0);

        let f_i = Field::Int(BigInt::from(-10));
        assert_eq!(f_i.as_biguint(), BigUint::zero());
        assert_eq!(f_i.as_f64(), -10.0);

        let f_b = Field::Bytes(b"hello".to_vec());
        assert_eq!(f_b.to_string(), "hello");
    }
}
