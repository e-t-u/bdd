use crate::error::BddError;
use crate::field::{reverse_bits, Field};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, ToPrimitive, Zero};
use rand::rngs::OsRng;
use rand::RngCore;
use std::sync::atomic::{AtomicU64, Ordering};

/// Returns the natural default bit width for a pattern character code when no number is specified.
pub fn default_bits_for_type(c: char, is_output: bool) -> usize {
    match c {
        'x' | 'X' => {
            if is_output {
                0
            } else {
                1
            }
        }
        'u' | 'U' | 'b' | 'z' | 'o' | 'r' => 1,
        'B' | 's' | 'S' | 'M' | 'q' | 'Q' | 'e' | 'E' | 'c' | 'C' | 'k' | 'K' => 8,
        'm' => 4,
        'h' | 'H' | 'y' | 'Y' => 16,
        'f' | 'F' => 32,
        'd' | 'D' => 64,
        _ => 1,
    }
}

/// A single token in a bitstream pattern specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternItem {
    pub name: Option<String>,
    pub bits: usize,
    pub char_code: char,
}

fn expand_single_token(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return String::new();
    }
    let (name, body) = if let Some(colon) = s.find(':') {
        let n = s[..colon].trim();
        let b = s[colon + 1..].trim();
        (
            if n.is_empty() {
                None
            } else {
                Some(n.to_string())
            },
            b,
        )
    } else {
        (None, s)
    };

    let mut out = String::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '(' {
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
            let expanded_inner = expand_pattern_multipliers(&inner);
            if let Some(ref n) = name {
                out.push_str(&format!("{}:{}", n, expanded_inner));
            } else {
                if !out.is_empty() && !out.ends_with(',') && expanded_inner.contains(',') {
                    out.push(',');
                }
                out.push_str(&expanded_inner);
            }
        } else if c.is_ascii_digit() {
            let mut num_str = String::new();
            num_str.push(c);
            while let Some(&next_c) = chars.peek() {
                if next_c.is_ascii_digit() {
                    num_str.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            // Optional whitespace before '*'
            let mut ws_before = String::new();
            while let Some(&next_c) = chars.peek() {
                if next_c.is_whitespace() {
                    ws_before.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if chars.peek() == Some(&'*') {
                chars.next(); // consume '*'
                              // Optional whitespace after '*'
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_whitespace() {
                        chars.next();
                    } else {
                        break;
                    }
                }
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
                    for i in 0..count {
                        if let Some(ref n) = name {
                            if count > 1 {
                                if !out.is_empty() && !out.ends_with(',') {
                                    out.push(',');
                                }
                                out.push_str(&format!("{}_{}:{}", n, i, expanded_inner));
                            } else {
                                out.push_str(&format!("{}:{}", n, expanded_inner));
                            }
                        } else {
                            if !out.is_empty()
                                && !out.ends_with(',')
                                && expanded_inner.contains(',')
                            {
                                out.push(',');
                            }
                            out.push_str(&expanded_inner);
                        }
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
                        for i in 0..count {
                            if let Some(ref n) = name {
                                if count > 1 {
                                    if !out.is_empty() && !out.ends_with(',') {
                                        out.push(',');
                                    }
                                    out.push_str(&format!("{}_{}:{}", n, i, token));
                                } else {
                                    out.push_str(&format!("{}:{}", n, token));
                                }
                            } else {
                                out.push_str(&token);
                            }
                        }
                    }
                }
            } else if let Some(ref n) = name {
                out.push_str(&format!("{}:{}{}", n, num_str, ws_before));
            } else {
                out.push_str(&num_str);
                out.push_str(&ws_before);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Expands repetition multipliers in pattern strings, e.g. "4*8B" -> "8B8B8B8B",
/// "w:4*6E" -> "w_0:6E,w_1:6E,w_2:6E,w_3:6E", and "2*(4U4U)" -> "4U4U4U4U".
pub fn expand_pattern_multipliers(s: &str) -> String {
    // Normalize spaces around '*' and ':'
    let mut normalized = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '*' || c == ':' {
            while normalized.ends_with(' ') || normalized.ends_with('\t') {
                normalized.pop();
            }
            normalized.push(c);
            i += 1;
            while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
                i += 1;
            }
        } else {
            normalized.push(c);
            i += 1;
        }
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    for c in normalized.chars() {
        if c == '(' {
            depth += 1;
            current.push(c);
        } else if c == ')' {
            if depth > 0 {
                depth -= 1;
            }
            current.push(c);
        } else if depth == 0 && (c == ',' || c == ';' || c.is_whitespace()) {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                tokens.push(trimmed.to_string());
                current.clear();
            }
        } else {
            current.push(c);
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        tokens.push(trimmed.to_string());
    }

    if tokens.is_empty() {
        return String::new();
    }

    let mut expanded_tokens = Vec::new();
    for t in tokens {
        expanded_tokens.push(expand_single_token(&t));
    }
    expanded_tokens.join(",")
}

/// Parse and validate an input pattern string (e.g. "2U3U5U", "sync:11u,version:2u", "32F", "4*8B", "16H", "8E").
pub fn parse_input_pattern(pattern_str: &str) -> Result<Vec<PatternItem>, BddError> {
    let expanded = expand_pattern_multipliers(pattern_str);
    let mut items = Vec::new();

    let tokens: Vec<&str> = expanded
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if tokens.is_empty() {
        return Err(BddError::MissingInputPattern);
    }

    for token in tokens {
        let (name, body) = if let Some(colon) = token.find(':') {
            let n = token[..colon].trim();
            let b = token[colon + 1..].trim();
            (
                if n.is_empty() {
                    None
                } else {
                    Some(n.to_string())
                },
                b,
            )
        } else {
            (None, token)
        };

        let mut digits = String::new();
        let mut is_first = true;
        for c in body.chars() {
            if c.is_ascii_digit() {
                digits.push(c);
            } else {
                let bits = if digits.is_empty() {
                    default_bits_for_type(c, false)
                } else {
                    digits.parse::<usize>().unwrap_or(0)
                };
                digits.clear();
                let item_name = if is_first {
                    is_first = false;
                    name.clone()
                } else {
                    None
                };
                items.push(PatternItem {
                    name: item_name,
                    bits,
                    char_code: c,
                });
            }
        }
        if !digits.is_empty() {
            return Err(BddError::MissingInputPattern);
        }
    }

    if items.is_empty() {
        return Err(BddError::MissingInputPattern);
    }

    for item in &items {
        let c = item.char_code;
        #[cfg(not(feature = "small-floats"))]
        if "HhYyQqEe".contains(c) {
            return Err(BddError::CliError(format!(
                "Small floating-point format '{}' requires the 'small-floats' feature to be enabled. Recompile with --features small-floats.",
                c
            )));
        }
        if !"xXUuBbSsMmFfDdHhYyEeQqCcKk".contains(c) {
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
            crate::diag::warn(format!(
                "Number of bits for input pattern {} should be n*8 bits",
                c
            ));
        }
    }

    Ok(items)
}

/// Parse and validate an output pattern string (e.g. "4z4M", "8u8U8u", "4*16H").
pub fn parse_output_pattern(pattern_str: &str) -> Result<Vec<PatternItem>, BddError> {
    let expanded = expand_pattern_multipliers(pattern_str);
    let mut items = Vec::new();

    let tokens: Vec<&str> = expanded
        .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if tokens.is_empty() {
        return Err(BddError::MissingOutputPattern);
    }

    for token in tokens {
        let body = if let Some(colon) = token.find(':') {
            token[colon + 1..].trim()
        } else {
            token
        };

        let mut digits = String::new();
        for c in body.chars() {
            if c.is_ascii_digit() {
                digits.push(c);
            } else {
                let bits = if digits.is_empty() {
                    default_bits_for_type(c, true)
                } else {
                    digits.parse::<usize>().unwrap_or(0)
                };
                digits.clear();
                items.push(PatternItem {
                    name: None,
                    bits,
                    char_code: c,
                });
            }
        }
        if !digits.is_empty() {
            return Err(BddError::MissingOutputPattern);
        }
    }

    if items.is_empty() {
        return Err(BddError::MissingOutputPattern);
    }

    for item in &items {
        let c = item.char_code;
        #[cfg(not(feature = "small-floats"))]
        if "HhYyQqEe".contains(c) {
            return Err(BddError::CliError(format!(
                "Small floating-point format '{}' requires the 'small-floats' feature to be enabled. Recompile with --features small-floats.",
                c
            )));
        }
        if !"UuBbSsMmFfDdHhYyEeQqCczorXxKk".contains(c) {
            return Err(BddError::IllegalOutputPatternChar(c));
        }
        if item.bits == 0 && c != 'x' && c != 'X' {
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
            crate::diag::warn(format!(
                "Number of bits for output pattern {} should be n*8 bits",
                c
            ));
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

    /// Returns field names for the unpacked tuple if any field in the pattern was named.
    pub fn field_names(&self) -> Option<Vec<String>> {
        let has_any_name = self.pattern_items.iter().any(|p| p.name.is_some());
        if !has_any_name {
            return None;
        }
        let mut names = Vec::new();
        let mut idx = 0;
        for p in &self.pattern_items {
            if p.char_code == 'x' || p.char_code == 'X' {
                continue;
            }
            if p.char_code == 'M' || p.char_code == 'm' {
                let base = p.name.clone().unwrap_or_else(|| format!("field_{}", idx));
                names.push(format!("{}_mag", base));
                names.push(format!("{}_sign", base));
                idx += 2;
            } else {
                let name = p.name.clone().unwrap_or_else(|| format!("field_{}", idx));
                names.push(name);
                idx += 1;
            }
        }
        Some(names)
    }

    pub fn unpack(&self, mut unit: BigUint) -> Vec<Field> {
        let mut tuple = Vec::new();
        for p in &self.reversed_pattern {
            let bits = p.bits;
            let c = p.char_code;
            if "usmfdchyqebk".contains(c) {
                unit = reverse_bits(&unit, bits);
            }
            if c == 'x' || c == 'X' {
                unit >>= bits;
            } else if c == 'U' || c == 'u' || c == 'B' || c == 'b' || c == 'K' || c == 'k' {
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
                #[cfg(feature = "small-floats")]
                {
                    let mask = (BigUint::one() << 16) - 1u32;
                    let val = &unit & &mask;
                    let u = val.to_u16().unwrap_or(0);
                    let fl = crate::float_types::decode_f16(u);
                    tuple.push(Field::Float(fl));
                    unit >>= 16;
                }
                #[cfg(not(feature = "small-floats"))]
                {
                    unit >>= 16;
                }
            } else if c == 'Y' || c == 'y' {
                #[cfg(feature = "small-floats")]
                {
                    let mask = (BigUint::one() << 16) - 1u32;
                    let val = &unit & &mask;
                    let u = val.to_u16().unwrap_or(0);
                    let fl = crate::float_types::decode_bf16(u);
                    tuple.push(Field::Float(fl));
                    unit >>= 16;
                }
                #[cfg(not(feature = "small-floats"))]
                {
                    unit >>= 16;
                }
            } else if c == 'Q' || c == 'q' {
                #[cfg(feature = "small-floats")]
                {
                    let mask = (BigUint::one() << 8) - 1u32;
                    let val = &unit & &mask;
                    let u = val.to_u8().unwrap_or(0);
                    let fl = crate::float_types::decode_fp8_e5m2(u);
                    tuple.push(Field::Float(fl));
                    unit >>= 8;
                }
                #[cfg(not(feature = "small-floats"))]
                {
                    unit >>= 8;
                }
            } else if c == 'E' || c == 'e' {
                #[cfg(feature = "small-floats")]
                {
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
                }
                #[cfg(not(feature = "small-floats"))]
                {
                    unit >>= bits;
                }
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
    counter: AtomicU64,
    random_bit_buffer: std::sync::Mutex<(BigUint, usize)>,
}

impl TuplePacker {
    pub fn new(pattern_str: &str) -> Result<Self, BddError> {
        let pattern = parse_output_pattern(pattern_str)?;
        let total_bits = pattern
            .iter()
            .filter(|p| p.char_code != 'x' && p.char_code != 'X')
            .map(|p| p.bits)
            .sum();
        Ok(Self {
            pattern,
            total_bits,
            counter: AtomicU64::new(0),
            random_bit_buffer: std::sync::Mutex::new((BigUint::zero(), 0)),
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

        for p in &self.pattern {
            let bits = p.bits;
            let c = p.char_code;
            if c == 'x' || c == 'X' {
                let _ = Self::pop_field(&mut tuple)?;
                continue;
            }
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
                #[cfg(feature = "small-floats")]
                'H' | 'h' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let u = crate::float_types::encode_f16(f.as_f64());
                    BigUint::from(u)
                }
                #[cfg(feature = "small-floats")]
                'Y' | 'y' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let u = crate::float_types::encode_bf16(f.as_f64());
                    BigUint::from(u)
                }
                #[cfg(feature = "small-floats")]
                'Q' | 'q' => {
                    let f = Self::pop_field(&mut tuple)?;
                    let u = crate::float_types::encode_fp8_e5m2(f.as_f64());
                    BigUint::from(u)
                }
                #[cfg(feature = "small-floats")]
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
                #[cfg(not(feature = "small-floats"))]
                'H' | 'h' | 'Y' | 'y' | 'Q' | 'q' | 'E' | 'e' => {
                    return Err(BddError::CliError(format!(
                        "Small floating-point format '{}' requires the 'small-floats' feature to be enabled. Recompile with --features small-floats.",
                        c
                    )));
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
                    if bits == 0 {
                        BigUint::zero()
                    } else {
                        let mut guard = self.random_bit_buffer.lock().unwrap();
                        while guard.1 < bits {
                            let needed_bits = bits - guard.1;
                            let needed_bytes = needed_bits.div_ceil(8);
                            let mut buf = vec![0u8; needed_bytes];
                            OsRng.fill_bytes(&mut buf);
                            let chunk = BigUint::from_bytes_be(&buf);
                            let chunk_bits = needed_bytes * 8;
                            guard.0 = (std::mem::take(&mut guard.0) << chunk_bits) | chunk;
                            guard.1 += chunk_bits;
                        }
                        let right_edge = guard.1 - bits;
                        let val = &guard.0 >> right_edge;
                        guard.1 -= bits;
                        let mask = if guard.1 > 0 {
                            (BigUint::one() << guard.1) - 1u32
                        } else {
                            BigUint::zero()
                        };
                        guard.0 &= mask;
                        val
                    }
                }
                'k' | 'K' => {
                    let cnt = self.counter.fetch_add(1, Ordering::Relaxed);
                    BigUint::from(cnt)
                }
                _ => BigUint::zero(),
            };

            if "usmfdchyqebk".contains(c) {
                val = reverse_bits(&val, bits);
            }
            let mask = if bits > 0 {
                (BigUint::one() << bits) - 1u32
            } else {
                BigUint::zero()
            };
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
                name: None,
                bits: 2,
                char_code: 'U'
            }
        );

        let named_p = parse_input_pattern("sync:11u,version:2u,layer:2u").unwrap();
        assert_eq!(named_p.len(), 3);
        assert_eq!(named_p[0].name.as_deref(), Some("sync"));
        assert_eq!(named_p[1].name.as_deref(), Some("version"));
        assert_eq!(named_p[2].name.as_deref(), Some("layer"));

        let unpacker_named = TupleUnpacker::new("sync:11u,version:2u").unwrap();
        assert_eq!(
            unpacker_named.field_names(),
            Some(vec!["sync".to_string(), "version".to_string()])
        );

        #[cfg(feature = "small-floats")]
        let mult_named = parse_input_pattern("w:4*6E").unwrap();
        #[cfg(not(feature = "small-floats"))]
        let mult_named = parse_input_pattern("w:4*6U").unwrap();
        assert_eq!(mult_named.len(), 4);
        assert_eq!(mult_named[0].name.as_deref(), Some("w_0"));
        assert_eq!(mult_named[3].name.as_deref(), Some("w_3"));

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
            parse_input_pattern("32Z"),
            Err(BddError::IllegalInputPatternChar('Z'))
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

        // Arbitrarily nested parentheses: 2*(2*(u2*U))
        let nested_2x2 = "2*(2*(u2*U))";
        assert_eq!(expand_pattern_multipliers(nested_2x2), "uUUuUUuUUuUU");
        let p_nested = parse_input_pattern(nested_2x2).unwrap();
        assert_eq!(p_nested.len(), 12);
        assert_eq!(p_nested.iter().map(|item| item.bits).sum::<usize>(), 12);

        // 3-level nesting: 2*(2*(2*(u2*U)))
        let nested_3 = "2*(2*(2*(u2*U)))";
        let p_nested3 = parse_input_pattern(nested_3).unwrap();
        assert_eq!(p_nested3.len(), 24);
        assert_eq!(p_nested3.iter().map(|item| item.bits).sum::<usize>(), 24);

        // Nested parentheses with commas: 2*(4U, 4u)
        let p_commas = parse_input_pattern("2*(4U, 4u)").unwrap();
        assert_eq!(p_commas.len(), 4);
        assert_eq!(p_commas[0].bits, 4);
        assert_eq!(p_commas[1].bits, 4);
        assert_eq!(p_commas[2].bits, 4);
        assert_eq!(p_commas[3].bits, 4);

        // Bare parentheses without multiplier: (u2*U)
        let p_bare = parse_input_pattern("(u2*U)").unwrap();
        assert_eq!(p_bare.len(), 3);
        assert_eq!(p_bare[0].char_code, 'u');
        assert_eq!(p_bare[1].char_code, 'U');
        assert_eq!(p_bare[2].char_code, 'U');

        // Parentheses with whitespace around operators: 2 * ( 2 * ( u 2*U ) )
        let p_spaces = parse_input_pattern("2 * ( 2 * ( u 2*U ) )").unwrap();
        assert_eq!(p_spaces.len(), 12);

        // Unpack / pack roundtrip with 2*(2*(u2*U))
        let unpacker = TupleUnpacker::new(nested_2x2).unwrap();
        assert_eq!(unpacker.total_bits, 12);
        let test_val = BigUint::from(0b101100110101u32);
        let tuple = unpacker.unpack(test_val.clone());
        assert_eq!(tuple.len(), 12);
        let packer = TuplePacker::new(nested_2x2).unwrap();
        let packed = packer.pack(tuple).unwrap();
        assert_eq!(packed, test_val);
    }

    #[cfg(feature = "small-floats")]
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
