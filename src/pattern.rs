use crate::error::BddError;
use crate::field::{reverse_bits, Field};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, ToPrimitive, Zero};
use rand::RngCore;

/// A single token in a bitstream pattern specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternItem {
    pub bits: usize,
    pub char_code: char,
}

/// Parse and validate an input pattern string (e.g. "2U3U5U", "32F").
pub fn parse_input_pattern(pattern_str: &str) -> Result<Vec<PatternItem>, BddError> {
    let mut items = Vec::new();
    let mut digits = String::new();
    let mut matched_len = 0;

    for c in pattern_str.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            let bits = if digits.is_empty() {
                0
            } else {
                digits.parse::<usize>().unwrap_or(0)
            };
            digits.clear();
            items.push(PatternItem { bits, char_code: c });
            matched_len += 1;
        }
    }

    if items.is_empty() || matched_len == 0 || !digits.is_empty() {
        return Err(BddError::MissingInputPattern);
    }

    for item in &items {
        let c = item.char_code;
        if !"xUuSsMmFfDdCc".contains(c) {
            return Err(BddError::IllegalInputPatternChar(c));
        }
        if item.bits == 0 {
            return Err(BddError::InputBitLengthRequired(c));
        }
        if "Ff".contains(c) && item.bits != 32 {
            return Err(BddError::InvalidInputBitLength(c, 32));
        }
        if "Dd".contains(c) && item.bits != 64 {
            return Err(BddError::InvalidInputBitLength(c, 64));
        }
        if "Cc".contains(c) && item.bits % 8 != 0 {
            eprintln!("Number of bits for input pattern {} should be n*8 bits", c);
        }
    }

    Ok(items)
}

/// Parse and validate an output pattern string (e.g. "4z4M", "8u8U8u").
pub fn parse_output_pattern(pattern_str: &str) -> Result<Vec<PatternItem>, BddError> {
    let mut items = Vec::new();
    let mut digits = String::new();

    for c in pattern_str.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            let bits = if digits.is_empty() {
                0
            } else {
                digits.parse::<usize>().unwrap_or(0)
            };
            digits.clear();
            items.push(PatternItem { bits, char_code: c });
        }
    }

    if items.is_empty() || !digits.is_empty() {
        return Err(BddError::MissingOutputPattern);
    }

    for item in &items {
        let c = item.char_code;
        if !"UuSsMmFfDdCczor".contains(c) {
            return Err(BddError::IllegalOutputPatternChar(c));
        }
        if item.bits == 0 {
            return Err(BddError::OutputBitLengthRequired(c));
        }
        if "Ff".contains(c) && item.bits != 32 {
            return Err(BddError::InvalidOutputBitLength(c, 32));
        }
        if "Dd".contains(c) && item.bits != 64 {
            return Err(BddError::InvalidOutputBitLength(c, 64));
        }
        if "Cc".contains(c) && item.bits % 8 != 0 {
            eprintln!("Number of bits for output pattern {} should be n*8 bits", c);
        }
    }

    Ok(items)
}

/// Unpacks a bitstream integer unit into individual fields according to an input pattern.
pub struct TupleUnpacker {
    reversed_pattern: Vec<PatternItem>,
    pub total_bits: usize,
}

impl TupleUnpacker {
    pub fn new(pattern_str: &str) -> Result<Self, BddError> {
        let pattern = parse_input_pattern(pattern_str)?;
        let total_bits = pattern.iter().map(|p| p.bits).sum();
        let mut reversed_pattern = pattern;
        reversed_pattern.reverse();
        Ok(Self {
            reversed_pattern,
            total_bits,
        })
    }

    pub fn unpack(&self, mut unit: BigUint) -> Vec<Field> {
        let mut tuple = Vec::new();
        for p in &self.reversed_pattern {
            let bits = p.bits;
            let c = p.char_code;
            if "usmfdc".contains(c) {
                unit = reverse_bits(&unit, bits);
            }
            if c == 'x' {
                unit >>= bits;
            } else if c == 'U' || c == 'u' {
                let mask = (BigUint::one() << bits) - 1u32;
                let val = &unit & &mask;
                tuple.push(Field::UInt(val));
                unit >>= bits;
            } else if c == 'S' || c == 's' {
                let mask = (BigUint::one() << bits) - 1u32;
                let mut val = &unit & &mask;
                let is_neg = ((&val >> (bits - 1)) & BigUint::one()) == BigUint::one();
                if is_neg {
                    val = (val ^ &mask) + 1u32;
                    tuple.push(Field::Int(-BigInt::from_biguint(Sign::Plus, val)));
                } else {
                    tuple.push(Field::Int(BigInt::from_biguint(Sign::Plus, val)));
                }
                unit >>= bits;
            } else if c == 'M' || c == 'm' {
                let mask = (BigUint::one() << bits) - 1u32;
                let mut val = &unit & &mask;
                let sign = ((&val >> (bits - 1)) & BigUint::one()).to_u8().unwrap_or(0);
                if sign == 1 {
                    val = (val ^ &mask) + 1u32;
                }
                tuple.push(Field::UInt(val));
                tuple.push(Field::UInt(BigUint::from(sign)));
                unit >>= bits;
            } else if c == 'F' || c == 'f' {
                let mask = (BigUint::one() << 32) - 1u32;
                let val = &unit & &mask;
                let u = val.to_u32().unwrap_or(0);
                let s3 = (u & 0xFF) as u8;
                let s2 = ((u >> 8) & 0xFF) as u8;
                let s1 = ((u >> 16) & 0xFF) as u8;
                let s0 = ((u >> 24) & 0xFF) as u8;
                let fl = f32::from_ne_bytes([s0, s1, s2, s3]);
                tuple.push(Field::Float(fl as f64));
                unit >>= 32;
            } else if c == 'D' || c == 'd' {
                let mask = (BigUint::one() << 64) - 1u32;
                let val = &unit & &mask;
                let u = val.to_u64().unwrap_or(0);
                let mut bytes = [0u8; 8];
                for idx in 0..8 {
                    bytes[7 - idx] = ((u >> (idx * 8)) & 0xFF) as u8;
                }
                let fl = f64::from_ne_bytes(bytes);
                tuple.push(Field::Float(fl));
                unit >>= 64;
            } else if c == 'C' || c == 'c' {
                let mut bytes = Vec::new();
                let mask = BigUint::from(0xFFu32);
                for _ in 0..(bits / 8) {
                    let b = (&unit & &mask).to_u8().unwrap_or(0);
                    bytes.push(b);
                    unit >>= 8;
                }
                bytes.reverse();
                tuple.push(Field::Bytes(bytes));
            }
        }
        tuple.reverse();
        tuple
    }
}

/// Packs tuple fields into an integer unit according to an output pattern.
pub struct TuplePacker {
    pattern: Vec<PatternItem>,
    pub total_bits: usize,
}

impl TuplePacker {
    pub fn new(pattern_str: &str) -> Result<Self, BddError> {
        let pattern = parse_output_pattern(pattern_str)?;
        let total_bits = pattern.iter().map(|p| p.bits).sum();
        Ok(Self {
            pattern,
            total_bits,
        })
    }

    fn pop_field(tuple: &mut Vec<Field>) -> Result<Field, BddError> {
        if tuple.is_empty() {
            return Err(BddError::InputHasLessFields);
        }
        Ok(tuple.remove(0))
    }

    pub fn pack(&self, mut tuple: Vec<Field>) -> Result<BigUint, BddError> {
        let mut unit = BigUint::zero();
        let mut rng = rand::thread_rng();

        for p in &self.pattern {
            let bits = p.bits;
            let c = p.char_code;
            let mut val = match c {
                'U' | 'u' => {
                    let f = Self::pop_field(&mut tuple)?;
                    f.as_biguint()
                }
                'S' | 's' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let bi = f.as_bigint();
                    if bi < BigInt::zero() {
                        let abs_u = (-bi).to_biguint().unwrap();
                        let mask = (BigUint::one() << bits) - 1u32;
                        let mut v = abs_u & &mask;
                        v ^= &mask;
                        v += 1u32;
                        v
                    } else {
                        bi.to_biguint().unwrap()
                    }
                }
                'M' | 'm' => {
                    let sign = Self::pop_field(&mut tuple)?.as_biguint();
                    let mut mag = Self::pop_field(&mut tuple)?.as_biguint();
                    if sign == BigUint::one() {
                        let mask = (BigUint::one() << bits) - 1u32;
                        mag &= &mask;
                        mag ^= &mask;
                        mag += 1u32;
                    }
                    mag
                }
                'F' | 'f' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let fl = f.as_f64() as f32;
                    let bytes = fl.to_ne_bytes();
                    let mut v = BigUint::zero();
                    for b in bytes {
                        v = (v << 8) | BigUint::from(b);
                    }
                    v
                }
                'D' | 'd' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let fl = f.as_f64();
                    let bytes = fl.to_ne_bytes();
                    let mut v = BigUint::zero();
                    for b in bytes {
                        v = (v << 8) | BigUint::from(b);
                    }
                    v
                }
                'C' | 'c' => {
                    let f = Self::pop_field(&mut tuple)?;
                    match f {
                        Field::Bytes(b) => {
                            let mut v = BigUint::zero();
                            for byte in b {
                                v = (v << 8) | BigUint::from(byte);
                            }
                            v
                        }
                        _ => f.as_biguint(),
                    }
                }
                'z' => BigUint::zero(),
                'o' => (BigUint::one() << bits) - 1u32,
                'r' => {
                    let num_bytes = bits / 8 + 1;
                    let mut buf = vec![0u8; num_bytes];
                    rng.fill_bytes(&mut buf);
                    BigUint::from_bytes_be(&buf)
                }
                _ => BigUint::zero(),
            };

            if "usmfdc".contains(c) {
                val = reverse_bits(&val, bits);
            }
            let mask = (BigUint::one() << bits) - 1u32;
            val &= mask;
            unit = (unit << bits) | val;
        }

        Ok(unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_patterns() {
        let p = parse_input_pattern("2U3U3U").unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(
            p[0],
            PatternItem {
                bits: 2,
                char_code: 'U'
            }
        );

        let unpacker = TupleUnpacker::new("8U").unwrap();
        assert_eq!(unpacker.total_bits, 8);
        let tuple = unpacker.unpack(BigUint::from(42u32));
        assert_eq!(tuple, vec![Field::UInt(BigUint::from(42u32))]);
    }

    #[test]
    fn test_invalid_patterns() {
        assert!(matches!(
            parse_input_pattern(""),
            Err(BddError::MissingInputPattern)
        ));
        assert!(matches!(
            parse_input_pattern("0U"),
            Err(BddError::InputBitLengthRequired('U'))
        ));
        assert!(matches!(
            parse_input_pattern("32X"),
            Err(BddError::IllegalInputPatternChar('X'))
        ));
        assert!(matches!(
            parse_input_pattern("16F"),
            Err(BddError::InvalidInputBitLength('F', 32))
        ));
    }

    #[test]
    fn test_packer_less_fields() {
        let packer = TuplePacker::new("8U8U").unwrap();
        let res = packer.pack(vec![Field::UInt(BigUint::from(1u32))]);
        assert_eq!(res, Err(BddError::InputHasLessFields));
    }
}
