use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, ToPrimitive, Zero};
use std::fmt;

/// Represents a single field value within a tuple.
#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    UInt(BigUint),
    Int(BigInt),
    Float(f64),
    Bytes(Vec<u8>),
}

impl Field {
    /// Convert this field to an unsigned big integer.
    pub fn as_biguint(&self) -> BigUint {
        match self {
            Field::UInt(u) => u.clone(),
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
                if let Ok(s) = std::str::from_utf8(bytes) {
                    if let Ok(u) = s.parse::<BigUint>() {
                        return u;
                    }
                }
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
            Field::Int(i) => i.clone(),
            Field::Float(f) => BigInt::from(*f as i64),
            Field::Bytes(bytes) => {
                if let Ok(s) = std::str::from_utf8(bytes) {
                    if let Ok(i) = s.parse::<BigInt>() {
                        return i;
                    }
                }
                BigInt::zero()
            }
        }
    }

    /// Convert this field to a 64-bit float.
    pub fn as_f64(&self) -> f64 {
        match self {
            Field::UInt(u) => u.to_f64().unwrap_or(0.0),
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
            let res = (high << bits) | rev;
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
    }

    #[test]
    fn test_reverse_bits_large() {
        let big = (BigUint::one() << 100) | BigUint::one();
        let rev = reverse_bits(&big, 101);
        // Bit 0 was 1 (now bit 100), bit 100 was 1 (now bit 0)
        assert_eq!(rev, big);
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
