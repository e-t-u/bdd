use crate::bits::BitValue;
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

    /// Convert this field to a 64-bit unsigned integer without heap allocation.
    #[inline]
    pub fn as_u64(&self) -> u64 {
        match self {
            Field::UInt(u) => u.to_u64().unwrap_or(0),
            Field::Bits(u, _) => u.to_u64().unwrap_or(0),
            Field::Int(i) => {
                if i >= &BigInt::zero() {
                    i.to_u64().unwrap_or(0)
                } else {
                    0
                }
            }
            Field::Float(f) => {
                if *f >= 0.0 {
                    *f as u64
                } else {
                    0
                }
            }
            Field::Bytes(bytes) => {
                let mut val = 0u64;
                for &b in bytes.iter().take(8) {
                    val = (val << 8) | (b as u64);
                }
                val
            }
        }
    }

    /// Convert this field to a BitValue without allocating if value fits in u64.
    pub fn as_bit_value(&self) -> BitValue {
        match self {
            Field::UInt(u) => {
                if let Some(v) = u.to_u64() {
                    BitValue::Inline(v)
                } else {
                    BitValue::Big(u.clone())
                }
            }
            Field::Bits(u, bits) => {
                if *bits <= 64 {
                    BitValue::Inline(u.to_u64().unwrap_or(0))
                } else {
                    BitValue::Big(u.clone())
                }
            }
            Field::Int(i) => {
                if i >= &BigInt::zero() {
                    if let Some(v) = i.to_u64() {
                        BitValue::Inline(v)
                    } else {
                        BitValue::Big(i.to_biguint().unwrap_or_default())
                    }
                } else {
                    BitValue::Inline(0)
                }
            }
            Field::Float(f) => {
                if *f >= 0.0 {
                    BitValue::Inline(*f as u64)
                } else {
                    BitValue::Inline(0)
                }
            }
            Field::Bytes(bytes) => {
                if bytes.len() <= 8 {
                    let mut val = 0u64;
                    for &b in bytes {
                        val = (val << 8) | (b as u64);
                    }
                    BitValue::Inline(val)
                } else {
                    BitValue::Big(self.as_biguint())
                }
            }
        }
    }

    /// Converts this field to a hashable key for deduplication and distinct count tracking.
    pub fn to_key(&self) -> FieldKey {
        match self {
            Field::UInt(u) => FieldKey::UInt(u.clone()),
            Field::Int(i) => FieldKey::Int(i.clone()),
            Field::Float(f) => FieldKey::FloatBits(f.to_bits()),
            Field::Bytes(b) => FieldKey::Bytes(b.clone()),
            Field::Bits(u, bits) => FieldKey::Bits(u.clone(), *bits),
        }
    }

    /// Returns the raw byte representation of this field.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Field::Bytes(b) => b.clone(),
            Field::UInt(u) => {
                let bytes = u.to_bytes_be();
                if bytes.is_empty() {
                    vec![0]
                } else {
                    bytes
                }
            }
            Field::Bits(u, bits) => {
                let bytes = u.to_bytes_be();
                let target_len = bits.div_ceil(8);
                if bytes.len() < target_len {
                    let mut padded = vec![0u8; target_len - bytes.len()];
                    padded.extend_from_slice(&bytes);
                    padded
                } else {
                    bytes
                }
            }
            Field::Int(i) => i.to_signed_bytes_be(),
            Field::Float(f) => f.to_be_bytes().to_vec(),
        }
    }

    /// Estimated or exact bit length of this field.
    pub fn bit_length(&self) -> usize {
        match self {
            Field::Bits(_, bits) => *bits,
            Field::Bytes(b) => b.len() * 8,
            Field::Float(_) => 64,
            Field::UInt(u) => (u.bits() as usize).max(1),
            Field::Int(i) => (i.bits() as usize).max(1),
        }
    }

    /// Compute the Hamming weight (number of set 1-bits) of this field.
    pub fn hamming_weight(&self) -> usize {
        match self {
            Field::Bytes(bytes) => bytes.iter().map(|b| b.count_ones() as usize).sum(),
            Field::Bits(u, _) | Field::UInt(u) => u
                .to_bytes_le()
                .iter()
                .map(|b| b.count_ones() as usize)
                .sum(),
            Field::Int(i) => i
                .to_signed_bytes_le()
                .iter()
                .map(|b| b.count_ones() as usize)
                .sum(),
            Field::Float(f) => f
                .to_be_bytes()
                .iter()
                .map(|b| b.count_ones() as usize)
                .sum(),
        }
    }

    /// Compute the bit balance (percentage of set 1-bits, 0.0% to 100.0%) of this field.
    pub fn bit_balance(&self) -> f64 {
        let total = self.bit_length();
        if total == 0 {
            0.0
        } else {
            (self.hamming_weight() as f64 / total as f64) * 100.0
        }
    }

    /// Compute the Shannon entropy of this field's byte representation (0.0 to 8.0 bits/byte).
    pub fn shannon_entropy(&self) -> f64 {
        crate::analysis::shannon_entropy(&self.to_bytes())
    }
}

/// Hashable identifier key for distinct counting and deduplication of field values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FieldKey {
    UInt(BigUint),
    Int(BigInt),
    FloatBits(u64),
    Bytes(Vec<u8>),
    Bits(BigUint, usize),
}

impl PartialOrd for Field {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Field::Float(a), b) => a.partial_cmp(&b.as_f64()),
            (a, Field::Float(b)) => a.as_f64().partial_cmp(b),
            (Field::Bytes(a), Field::Bytes(b)) => a.partial_cmp(b),
            (a, b) => a.as_bigint().partial_cmp(&b.as_bigint()),
        }
    }
}

impl From<BitValue> for Field {
    fn from(bv: BitValue) -> Self {
        match bv {
            BitValue::Inline(u) => Field::UInt(BigUint::from(u)),
            BitValue::Big(b) => Field::UInt(b),
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

/// Reverses the lowest `bits` bits of a 64-bit value using native CPU instructions.
#[inline]
pub fn reverse_bits_u64(val: u64, bits: usize) -> u64 {
    if bits == 0 {
        return val;
    }
    let high = if bits >= 64 { 0 } else { val >> bits };
    let mask = if bits >= 64 {
        !0u64
    } else {
        (1u64 << bits) - 1
    };
    let low = val & mask;
    let rev = low.reverse_bits() >> (64 - bits);
    if bits >= 64 {
        rev
    } else {
        (high << bits) | rev
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
            return BigUint::from(reverse_bits_u64(v, bits));
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

    #[test]
    fn test_field_analysis_and_ordering() {
        // PartialOrd
        let f1 = Field::UInt(BigUint::from(10u32));
        let f2 = Field::UInt(BigUint::from(20u32));
        assert!(f1 < f2);

        let ff1 = Field::Float(1.5);
        let ff2 = Field::Float(2.5);
        assert!(ff1 < ff2);
        assert!(ff1 < f1);

        // Analysis methods
        let f_zero = Field::Bytes(vec![0u8; 32]);
        assert_eq!(f_zero.shannon_entropy(), 0.0);
        assert_eq!(f_zero.hamming_weight(), 0);
        assert_eq!(f_zero.bit_balance(), 0.0);

        let f_ones = Field::Bytes(vec![0xFFu8; 32]);
        assert_eq!(f_ones.hamming_weight(), 256);
        assert_eq!(f_ones.bit_balance(), 100.0);

        let f_alt = Field::Bytes(vec![0xAAu8; 10]);
        assert_eq!(f_alt.bit_balance(), 50.0);
    }
}
