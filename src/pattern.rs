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

/// Expands repetition multipliers in pattern strings, e.g. "4*8B" -> "8B8B8B8B"
/// and "2*(4U4U)" -> "4U4U4U4U".
pub fn expand_pattern_multipliers(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            let mut num_str = String::new();
            num_str.push(c);
            while let Some(&next_c) = chars.peek() {
                if next_c.is_ascii_digit() {
                    num_str.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if chars.peek() == Some(&'*') {
                chars.next(); // consume '*'
                if chars.peek() == Some(&'(') {
                    chars.next(); // consume '('
                    let mut inner = String::new();
                    let mut depth = 1;
                    for ic in chars.by_ref() {
                        if ic == '(' {
                            depth += 1;
                            inner.push(ic);
                        } else if ic == ')' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            } else {
                                inner.push(ic);
                            }
                        } else {
                            inner.push(ic);
                        }
                    }
                    let count: usize = num_str.parse().unwrap_or(1);
                    let expanded_inner = expand_pattern_multipliers(&inner);
                    for _ in 0..count {
                        out.push_str(&expanded_inner);
                    }
                } else {
                    let mut token_digits = String::new();
                    while let Some(&next_c) = chars.peek() {
                        if next_c.is_ascii_digit() {
                            token_digits.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }
                    if let Some(letter) = chars.next() {
                        let count: usize = num_str.parse().unwrap_or(1);
                        let token = format!("{}{}", token_digits, letter);
                        for _ in 0..count {
                            out.push_str(&token);
                        }
                    }
                }
            } else {
                out.push_str(&num_str);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse and validate an input pattern string (e.g. "2U3U5U", "32F", "4*8B", "16H", "8E").
pub fn parse_input_pattern(pattern_str: &str) -> Result<Vec<PatternItem>, BddError> {
    let expanded = expand_pattern_multipliers(pattern_str);
    let mut items = Vec::new();
    let mut digits = String::new();
    let mut matched_len = 0;

    for c in expanded.chars() {
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
        if !"xUuBbSsMmFfDdHhYyEeQqCc".contains(c) {
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
        if "Hh".contains(c) && item.bits != 16 {
            return Err(BddError::InvalidInputBitLength(c, 16));
        }
        if "Yy".contains(c) && item.bits != 16 {
            return Err(BddError::InvalidInputBitLength(c, 16));
        }
        if "Qq".contains(c) && item.bits != 8 {
            return Err(BddError::InvalidInputBitLength(c, 8));
        }
        if "Ee".contains(c) && item.bits != 8 && item.bits != 6 && item.bits != 4 {
            return Err(BddError::InvalidInputBitLength(c, 8));
        }
        if "Cc".contains(c) && item.bits % 8 != 0 {
            eprintln!("Number of bits for input pattern {} should be n*8 bits", c);
        }
    }

    Ok(items)
}

/// Parse and validate an output pattern string (e.g. "4z4M", "8u8U8u", "4*16H").
pub fn parse_output_pattern(pattern_str: &str) -> Result<Vec<PatternItem>, BddError> {
    let expanded = expand_pattern_multipliers(pattern_str);
    let mut items = Vec::new();
    let mut digits = String::new();

    for c in expanded.chars() {
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
        if !"UuBbSsMmFfDdHhYyEeQqCczor".contains(c) {
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
        if "Hh".contains(c) && item.bits != 16 {
            return Err(BddError::InvalidOutputBitLength(c, 16));
        }
        if "Yy".contains(c) && item.bits != 16 {
            return Err(BddError::InvalidOutputBitLength(c, 16));
        }
        if "Qq".contains(c) && item.bits != 8 {
            return Err(BddError::InvalidOutputBitLength(c, 8));
        }
        if "Ee".contains(c) && item.bits != 8 && item.bits != 6 && item.bits != 4 {
            return Err(BddError::InvalidOutputBitLength(c, 8));
        }
        if "Cc".contains(c) && item.bits % 8 != 0 {
            eprintln!("Number of bits for output pattern {} should be n*8 bits", c);
        }
    }

    Ok(items)
}

/// Unpacks a bitstream integer unit into individual fields according to an input pattern.
pub struct TupleUnpacker {
    pub pattern_items: Vec<PatternItem>,
    reversed_pattern: Vec<PatternItem>,
    pub total_bits: usize,
}

impl TupleUnpacker {
    pub fn new(pattern_str: &str) -> Result<Self, BddError> {
        let pattern = parse_input_pattern(pattern_str)?;
        let total_bits = pattern.iter().map(|p| p.bits).sum();
        let mut reversed_pattern = pattern.clone();
        reversed_pattern.reverse();
        Ok(Self {
            pattern_items: pattern,
            reversed_pattern,
            total_bits,
        })
    }

    pub fn unpack(&self, mut unit: BigUint) -> Vec<Field> {
        let mut tuple = Vec::new();
        for p in &self.reversed_pattern {
            let bits = p.bits;
            let c = p.char_code;
            if "usmfdchyqeb".contains(c) {
                unit = reverse_bits(&unit, bits);
            }
            if c == 'x' {
                unit >>= bits;
            } else if c == 'U' || c == 'u' || c == 'B' || c == 'b' {
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
                let fl = f32::from_bits(u);
                tuple.push(Field::Float(fl as f64));
                unit >>= 32;
            } else if c == 'D' || c == 'd' {
                let mask = (BigUint::one() << 64) - 1u32;
                let val = &unit & &mask;
                let u = val.to_u64().unwrap_or(0);
                let fl = f64::from_bits(u);
                tuple.push(Field::Float(fl));
                unit >>= 64;
            } else if c == 'H' || c == 'h' {
                let mask = (BigUint::one() << 16) - 1u32;
                let val = &unit & &mask;
                let u = val.to_u16().unwrap_or(0);
                let fl = crate::float_types::decode_f16(u);
                tuple.push(Field::Float(fl));
                unit >>= 16;
            } else if c == 'Y' || c == 'y' {
                let mask = (BigUint::one() << 16) - 1u32;
                let val = &unit & &mask;
                let u = val.to_u16().unwrap_or(0);
                let fl = crate::float_types::decode_bf16(u);
                tuple.push(Field::Float(fl));
                unit >>= 16;
            } else if c == 'Q' || c == 'q' {
                let mask = (BigUint::one() << 8) - 1u32;
                let val = &unit & &mask;
                let u = val.to_u8().unwrap_or(0);
                let fl = crate::float_types::decode_fp8_e5m2(u);
                tuple.push(Field::Float(fl));
                unit >>= 8;
            } else if c == 'E' || c == 'e' {
                let mask = (BigUint::one() << bits) - 1u32;
                let val = &unit & &mask;
                let u = val.to_u8().unwrap_or(0);
                let fl = match bits {
                    8 => crate::float_types::decode_fp8_e4m3(u),
                    6 => crate::float_types::decode_fp6_e3m2(u),
                    4 => crate::float_types::decode_fp4_e2m1(u),
                    _ => 0.0,
                };
                tuple.push(Field::Float(fl));
                unit >>= bits;
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
                'U' | 'u' | 'B' | 'b' => {
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
                    BigUint::from(fl.to_bits())
                }
                'D' | 'd' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let fl = f.as_f64();
                    BigUint::from(fl.to_bits())
                }
                'H' | 'h' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let u = crate::float_types::encode_f16(f.as_f64());
                    BigUint::from(u)
                }
                'Y' | 'y' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let u = crate::float_types::encode_bf16(f.as_f64());
                    BigUint::from(u)
                }
                'Q' | 'q' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let u = crate::float_types::encode_fp8_e5m2(f.as_f64());
                    BigUint::from(u)
                }
                'E' | 'e' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let fl = f.as_f64();
                    let u = match bits {
                        8 => crate::float_types::encode_fp8_e4m3(fl),
                        6 => crate::float_types::encode_fp6_e3m2(fl),
                        4 => crate::float_types::encode_fp4_e2m1(fl),
                        _ => 0,
                    };
                    BigUint::from(u)
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

            if "usmfdchyqeb".contains(c) {
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

    #[test]
    fn test_large_units() {
        // Test 1024-bit and 4096-bit unit packing and unpacking
        let unpacker = TupleUnpacker::new("1024U").unwrap();
        assert_eq!(unpacker.total_bits, 1024);

        let big_val: BigUint =
            (BigUint::one() << 1023usize) | (BigUint::one() << 500usize) | BigUint::from(12345u32);
        let tuple = unpacker.unpack(big_val.clone());
        assert_eq!(tuple.len(), 1);
        assert_eq!(tuple[0], Field::UInt(big_val.clone()));

        let packer = TuplePacker::new("1024U").unwrap();
        let packed = packer.pack(tuple).unwrap();
        assert_eq!(packed, big_val);

        // 4096-bit unit
        let unpacker_4096 = TupleUnpacker::new("4096U").unwrap();
        assert_eq!(unpacker_4096.total_bits, 4096);
        let big_4096: BigUint = (BigUint::one() << 4095usize) | BigUint::one();
        let tuple_4096 = unpacker_4096.unpack(big_4096.clone());
        assert_eq!(tuple_4096[0], Field::UInt(big_4096.clone()));
        let packer_4096 = TuplePacker::new("4096U").unwrap();
        assert_eq!(packer_4096.pack(tuple_4096).unwrap(), big_4096);
    }

    #[test]
    fn test_large_tuple_size() {
        // 1000 fields of 1U = 1000 bits total
        let pattern = "1U".repeat(1000);
        let unpacker = TupleUnpacker::new(&pattern).unwrap();
        assert_eq!(unpacker.total_bits, 1000);

        // Value with alternating bits (even bits 1, odd bits 0)
        let mut big_val = BigUint::zero();
        for i in 0..1000 {
            if i % 2 == 0 {
                big_val |= BigUint::one() << i;
            }
        }

        let tuple = unpacker.unpack(big_val.clone());
        assert_eq!(tuple.len(), 1000);
        for (i, field) in tuple.iter().enumerate() {
            // Note: Tuple unpacker reads from MSB to LSB.
            // bit (999 - i)
            let bit_idx = 999 - i;
            let expected = if bit_idx % 2 == 0 { 1u32 } else { 0u32 };
            assert_eq!(*field, Field::UInt(BigUint::from(expected)));
        }

        let packer = TuplePacker::new(&pattern).unwrap();
        let packed = packer.pack(tuple).unwrap();
        assert_eq!(packed, big_val);
    }

    #[test]
    fn test_pattern_multipliers() {
        assert_eq!(expand_pattern_multipliers("4*8B"), "8B8B8B8B");
        assert_eq!(expand_pattern_multipliers("2*(4U4u)"), "4U4u4U4u");
        let p = parse_input_pattern("4*8B").unwrap();
        assert_eq!(p.len(), 4);
        assert_eq!(p.iter().map(|item| item.bits).sum::<usize>(), 32);
    }

    #[test]
    fn test_ai_floats_roundtrip() {
        // FP16 (16H), BF16 (16Y), FP8 E4M3 (8E), FP8 E5M2 (8Q), FP4 E2M1 (4E)
        let pattern = "16H16Y8E8Q4E";
        let packer = TuplePacker::new(pattern).unwrap();
        assert_eq!(packer.total_bits, 16 + 16 + 8 + 8 + 4);

        let tuple = vec![
            Field::Float(1.0),
            Field::Float(-1.0),
            Field::Float(2.0),
            Field::Float(-2.0),
            Field::Float(0.5),
        ];

        let packed = packer.pack(tuple).unwrap();
        let unpacker = TupleUnpacker::new(pattern).unwrap();
        let unpacked = unpacker.unpack(packed);

        assert_eq!(unpacked.len(), 5);
        assert_eq!(unpacked[0].as_f64(), 1.0);
        assert_eq!(unpacked[1].as_f64(), -1.0);
        assert_eq!(unpacked[2].as_f64(), 2.0);
        assert_eq!(unpacked[3].as_f64(), -2.0);
        assert_eq!(unpacked[4].as_f64(), 0.5);
    }
}
