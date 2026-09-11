use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Zero};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Field {
    UInt(BigUint),
    Int(BigInt),
    Float(f64),
    Bytes(Vec<u8>),
}

impl Field {
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

    pub fn as_f64(&self) -> f64 {
        match self {
            Field::UInt(u) => {
                use num_traits::ToPrimitive;
                u.to_f64().unwrap_or(0.0)
            }
            Field::Int(i) => {
                use num_traits::ToPrimitive;
                i.to_f64().unwrap_or(0.0)
            }
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

pub fn reverse_bits(val: &BigUint, bits: usize) -> BigUint {
    if bits == 0 {
        return val.clone();
    }
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
