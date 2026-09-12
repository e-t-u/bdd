use crate::cli::parse_size_with_suffix;
use crate::error::BddError;

/// Parsed specification for either the input or output side of a stream.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StreamSpec {
    pub skip: Option<u64>,
    pub raw_unit: Option<u64>,
    pub offset: Option<u64>,
    pub unit_size: Option<usize>,
    pub pattern: Option<String>,
    pub gap: Option<u64>,
}

/// Parsed stream I/O pattern combining input and output specifications.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StreamIoPattern {
    pub input: Option<StreamSpec>,
    pub output: Option<StreamSpec>,
}

/// Check if a string looks like a stream I/O pattern (vs a tuple pattern).
pub fn is_stream_io_pattern(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    // Explicit stream arrow
    if s.contains("->") {
        return true;
    }
    // Container brackets
    if s.contains('[') && s.contains(']') {
        return true;
    }
    // Stream gap operator '+' outside pattern
    if s.contains('+') {
        return true;
    }
    // If it has a colon ':' followed by a container or number, e.g. "123:8" or "5B:8[2:4]"
    if let Some(idx) = s.find(':') {
        let prefix = s[..idx].trim();
        // If prefix is numeric or size (e.g. 123, 5B, 4k), it's a stream skip!
        if parse_size_with_suffix(prefix, "check", true).is_ok() {
            return true;
        }
    }
    // Pure numbers / sizes without tuple pattern letters: e.g. "8", "16", "188B", "1024*8"
    if parse_size_with_suffix(s, "check", true).is_ok() {
        // If it's pure number or byte/kilo size, consider it a stream unit specification
        return true;
    }
    false
}

/// Parse a full stream I/O pattern, e.g. "123:8[2:4]+8 -> 5B:8[2:4]" or "8->3".
pub fn parse_stream_io_pattern(input_str: &str) -> Result<StreamIoPattern, BddError> {
    let trimmed = input_str.trim();
    if trimmed.is_empty() {
        return Ok(StreamIoPattern::default());
    }

    // Split on top-level '->' (outside any brackets)
    let mut depth = 0;
    let mut arrow_pos = None;
    let chars: Vec<(usize, char)> = trimmed.char_indices().collect();
    let n = chars.len();

    let mut i = 0;
    while i < n {
        let (byte_idx, c) = chars[i];
        match c {
            '[' => depth += 1,
            ']' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            '-' if depth == 0 && i + 1 < n && chars[i + 1].1 == '>' => {
                arrow_pos = Some(byte_idx);
                break;
            }
            _ => {}
        }
        i += 1;
    }

    if let Some(idx) = arrow_pos {
        let left = trimmed[..idx].trim();
        let right = trimmed[idx + 2..].trim();
        let in_spec = if !left.is_empty() {
            Some(parse_stream_spec(left)?)
        } else {
            None
        };
        let out_spec = if !right.is_empty() {
            Some(parse_stream_spec(right)?)
        } else {
            None
        };
        Ok(StreamIoPattern {
            input: in_spec,
            output: out_spec,
        })
    } else {
        let in_spec = Some(parse_stream_spec(trimmed)?);
        Ok(StreamIoPattern {
            input: in_spec,
            output: None,
        })
    }
}

/// Parse a single stream specification (input or output).
pub fn parse_stream_spec(s: &str) -> Result<StreamSpec, BddError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(StreamSpec::default());
    }

    // Support legacy 5-colon positional syntax if exactly 4 colons without brackets:
    // skip : raw : offset : unit : gap
    if !s.contains('[') {
        let colon_parts: Vec<&str> = s.split(':').map(|p| p.trim()).collect();
        if colon_parts.len() == 5 {
            let skip = Some(parse_size_with_suffix(colon_parts[0], "stream skip", true)?);
            let raw = Some(parse_size_with_suffix(
                colon_parts[1],
                "stream raw unit",
                true,
            )?);
            let offset = Some(parse_size_with_suffix(
                colon_parts[2],
                "stream offset",
                true,
            )?);
            let (unit_size, pattern) = parse_unit_or_pattern(colon_parts[3])?;
            let gap = Some(parse_size_with_suffix(colon_parts[4], "stream gap", true)?);
            return Ok(StreamSpec {
                skip,
                raw_unit: raw,
                offset,
                unit_size,
                pattern,
                gap,
            });
        }
    }

    // 1. Find periodic gap ('+' outside brackets)
    let mut depth = 0;
    let mut plus_pos = None;
    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            '+' if depth == 0 => plus_pos = Some(i),
            _ => {}
        }
    }

    let (s_remaining, gap) = if let Some(idx) = plus_pos {
        let gap_str = s[idx + 1..].trim();
        let gap_val = parse_size_with_suffix(gap_str, "stream gap", true)?;
        (s[..idx].trim(), Some(gap_val))
    } else {
        (s, None)
    };

    // 2. Find initial skip (':' outside brackets)
    let mut depth = 0;
    let mut colon_pos = None;
    for (i, c) in s_remaining.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            ':' if depth == 0 => {
                colon_pos = Some(i);
                break;
            }
            _ => {}
        }
    }

    let (unit_part, skip) = if let Some(idx) = colon_pos {
        let skip_str = s_remaining[..idx].trim();
        let skip_val = parse_size_with_suffix(skip_str, "stream skip", true)?;
        (s_remaining[idx + 1..].trim(), Some(skip_val))
    } else {
        (s_remaining, None)
    };

    // 3. Parse unit_part (Form A, Form B, or Bare Unit)
    let unit_part = unit_part.trim();
    if unit_part.is_empty() {
        return Ok(StreamSpec {
            skip,
            gap,
            ..Default::default()
        });
    }

    if let (Some(open_idx), Some(close_idx)) = (unit_part.find('['), unit_part.rfind(']')) {
        if open_idx > close_idx {
            return Err(BddError::CliError(format!(
                "Mismatched brackets in stream pattern: '{}'",
                unit_part
            )));
        }
        let prefix = unit_part[..open_idx].trim();
        let inside = unit_part[open_idx + 1..close_idx].trim();
        let inner_parts: Vec<&str> = if inside.contains(':') {
            inside.split(':').map(|p| p.trim()).collect()
        } else {
            inside.split(',').map(|p| p.trim()).collect()
        };

        if !prefix.is_empty() {
            // Form A: raw_size[offset : unit] or raw_size[offset : unit : post]
            let raw_val = parse_size_with_suffix(prefix, "raw container size", true)?;
            if inner_parts.len() < 2 || inner_parts.len() > 3 {
                return Err(BddError::CliError(format!(
                    "Form A container requires [offset:unit] or [offset:unit:post], found '[{}]'",
                    inside
                )));
            }
            let offset_val = parse_size_with_suffix(inner_parts[0], "container offset", true)?;
            let (unit_size, pattern) = parse_unit_or_pattern(inner_parts[1])?;
            let u_val = unit_size.unwrap_or(8) as u64;

            if offset_val + u_val > raw_val {
                return Err(BddError::CliError(format!(
                    "Container offset ({}) + unit ({}) exceeds raw container size ({})",
                    offset_val, u_val, raw_val
                )));
            }

            let _post_gap = if inner_parts.len() == 3 {
                let post = parse_size_with_suffix(inner_parts[2], "container post-gap", true)?;
                if offset_val + u_val + post != raw_val {
                    return Err(BddError::CliError(format!(
                        "Container components (offset: {}, unit: {}, post: {}) do not sum to raw container size ({})",
                        offset_val, u_val, post, raw_val
                    )));
                }
                post
            } else {
                raw_val - offset_val - u_val
            };

            // Post-gap is internal to the raw unit container;
            // The effective gap between extracted units is post_gap + stream_gap (if any).
            // We store raw_unit, offset, unit_size, and external gap.
            Ok(StreamSpec {
                skip,
                raw_unit: Some(raw_val),
                offset: Some(offset_val),
                unit_size,
                pattern,
                gap,
            })
        } else {
            // Form B: [pre : unit : post] or [pre : unit]
            if inner_parts.len() < 2 || inner_parts.len() > 3 {
                return Err(BddError::CliError(format!(
                    "Form B container requires [pre:unit:post] or [pre:unit], found '[{}]'",
                    inside
                )));
            }
            let pre_val = parse_size_with_suffix(inner_parts[0], "pre-gap", true)?;
            let (unit_size, pattern) = parse_unit_or_pattern(inner_parts[1])?;
            let u_val = unit_size.unwrap_or(8) as u64;
            let post_val = if inner_parts.len() == 3 {
                parse_size_with_suffix(inner_parts[2], "post-gap", true)?
            } else {
                0
            };
            let raw_val = pre_val + u_val + post_val;

            Ok(StreamSpec {
                skip,
                raw_unit: Some(raw_val),
                offset: Some(pre_val),
                unit_size,
                pattern,
                gap,
            })
        }
    } else {
        // Bare unit or pattern
        let (unit_size, pattern) = parse_unit_or_pattern(unit_part)?;
        Ok(StreamSpec {
            skip,
            raw_unit: None,
            offset: None,
            unit_size,
            pattern,
            gap,
        })
    }
}

/// Disambiguate and parse a unit spec: either a numeric size (e.g. "8", "16", "188B")
/// or a pattern string (e.g. "8U", "11u,5x", "24S").
fn parse_unit_or_pattern(s: &str) -> Result<(Option<usize>, Option<String>), BddError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok((None, None));
    }

    // If it contains commas or pattern type characters (U, u, S, s, etc.), try pattern parsing first
    let has_pattern_chars = s.contains(',')
        || s.chars().any(|c| {
            matches!(
                c,
                'U' | 'u'
                    | 'S'
                    | 's'
                    | 'M'
                    | 'm'
                    | 'F'
                    | 'f'
                    | 'D'
                    | 'd'
                    | 'E'
                    | 'e'
                    | 'Q'
                    | 'q'
                    | 'H'
                    | 'h'
                    | 'Y'
                    | 'y'
                    | 'C'
                    | 'c'
                    | 'K'
                    | 'k'
                    | 'z'
                    | 'o'
                    | 'r'
                    | 'x'
                    | 'X'
            )
        });

    if has_pattern_chars {
        // Try unpacking pattern grammar
        if let Ok(unpacker) = crate::pattern::TupleUnpacker::new(s) {
            return Ok((Some(unpacker.total_bits), Some(s.to_string())));
        }
    }

    // Try parsing as numeric size
    match parse_size_with_suffix(s, "unit size", true) {
        Ok(val) => Ok((Some(val as usize), None)),
        Err(e) => {
            // Fallback check: could it be a bare pattern?
            if let Ok(unpacker) = crate::pattern::TupleUnpacker::new(s) {
                Ok((Some(unpacker.total_bits), Some(s.to_string())))
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_arrow() {
        let p = parse_stream_io_pattern("8 -> 3").unwrap();
        assert_eq!(p.input.unwrap().unit_size, Some(8));
        assert_eq!(p.output.unwrap().unit_size, Some(3));
    }

    #[test]
    fn test_skip_and_gap() {
        let p = parse_stream_io_pattern("123 : 8 + 8 -> 3").unwrap();
        let inp = p.input.unwrap();
        assert_eq!(inp.skip, Some(123));
        assert_eq!(inp.unit_size, Some(8));
        assert_eq!(inp.gap, Some(8));
        assert_eq!(p.output.unwrap().unit_size, Some(3));
    }

    #[test]
    fn test_form_a_container() {
        let p = parse_stream_io_pattern("123 : 8[2:4] + 8 -> 5B : 8[2:4]").unwrap();
        let inp = p.input.unwrap();
        assert_eq!(inp.skip, Some(123));
        assert_eq!(inp.raw_unit, Some(8));
        assert_eq!(inp.offset, Some(2));
        assert_eq!(inp.unit_size, Some(4));
        assert_eq!(inp.gap, Some(8));

        let out = p.output.unwrap();
        assert_eq!(out.skip, Some(40)); // 5 bytes = 40 bits
        assert_eq!(out.raw_unit, Some(8));
        assert_eq!(out.offset, Some(2));
        assert_eq!(out.unit_size, Some(4));
        assert_eq!(out.gap, None);
    }

    #[test]
    fn test_form_b_container() {
        let p = parse_stream_io_pattern("123 : [2:4:2] + 8 -> 5B : [2:4:2]").unwrap();
        let inp = p.input.unwrap();
        assert_eq!(inp.skip, Some(123));
        assert_eq!(inp.raw_unit, Some(8)); // 2 + 4 + 2
        assert_eq!(inp.offset, Some(2));
        assert_eq!(inp.unit_size, Some(4));
        assert_eq!(inp.gap, Some(8));

        let out = p.output.unwrap();
        assert_eq!(out.skip, Some(40));
        assert_eq!(out.raw_unit, Some(8));
        assert_eq!(out.offset, Some(2));
        assert_eq!(out.unit_size, Some(4));
    }

    #[test]
    fn test_mpeg_ts_container() {
        let p = parse_stream_io_pattern("188B[11:13] -> 13").unwrap();
        let inp = p.input.unwrap();
        assert_eq!(inp.raw_unit, Some(1504));
        assert_eq!(inp.offset, Some(11));
        assert_eq!(inp.unit_size, Some(13));
        assert_eq!(p.output.unwrap().unit_size, Some(13));
    }

    #[test]
    fn test_repack_mpeg_ts() {
        let p = parse_stream_io_pattern("13 -> 188B[11:13]").unwrap();
        assert_eq!(p.input.unwrap().unit_size, Some(13));
        let out = p.output.unwrap();
        assert_eq!(out.raw_unit, Some(1504));
        assert_eq!(out.offset, Some(11));
        assert_eq!(out.unit_size, Some(13));
    }

    #[test]
    fn test_colon_5_tuple_fallback() {
        let p = parse_stream_io_pattern("123:8:2:4:8 -> 5B:8:2:4:0").unwrap();
        let inp = p.input.unwrap();
        assert_eq!(inp.skip, Some(123));
        assert_eq!(inp.raw_unit, Some(8));
        assert_eq!(inp.offset, Some(2));
        assert_eq!(inp.unit_size, Some(4));
        assert_eq!(inp.gap, Some(8));

        let out = p.output.unwrap();
        assert_eq!(out.skip, Some(40));
        assert_eq!(out.raw_unit, Some(8));
        assert_eq!(out.offset, Some(2));
        assert_eq!(out.unit_size, Some(4));
        assert_eq!(out.gap, Some(0));
    }

    #[test]
    fn test_stream_multipliers_and_large_units() {
        // 1GB skip (decimal 1000^3 * 8 bits = 8,000,000,000 bits),
        // container size 8 bits with slice [2, 6] (offset 2, unit 6),
        // periodic gap +1MiB (binary 1024^2 * 8 bits = 8,388,608 bits)
        let p = parse_stream_io_pattern("1GB:8[2,6]+1MiB").unwrap();
        let inp = p.input.unwrap();
        assert_eq!(inp.skip, Some(8_000_000_000));
        assert_eq!(inp.raw_unit, Some(8));
        assert_eq!(inp.offset, Some(2));
        assert_eq!(inp.unit_size, Some(6));
        assert_eq!(inp.gap, Some(8_388_608));

        // Colon slice syntax 8[2:6] with binary 1GiB
        let p2 = parse_stream_io_pattern("1GiB:8[2:6]+100k").unwrap();
        let inp2 = p2.input.unwrap();
        assert_eq!(inp2.skip, Some(1024 * 1024 * 1024 * 8));
        assert_eq!(inp2.raw_unit, Some(8));
        assert_eq!(inp2.offset, Some(2));
        assert_eq!(inp2.unit_size, Some(6));
        assert_eq!(inp2.gap, Some(100 * 1024));

        // Multiplication expressions in raw size, skip, and gap
        let p3 = parse_stream_io_pattern("1024*1024*8 : 188*8[0:32] + 100*8").unwrap();
        let inp3 = p3.input.unwrap();
        assert_eq!(inp3.skip, Some(1024 * 1024 * 8));
        assert_eq!(inp3.raw_unit, Some(1504));
        assert_eq!(inp3.offset, Some(0));
        assert_eq!(inp3.unit_size, Some(32));
        assert_eq!(inp3.gap, Some(800));
    }
}
