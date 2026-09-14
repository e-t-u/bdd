use crate::cli::parse_size_with_suffix;
use crate::error::BddError;
use crate::pattern::FramedPattern;

/// Parsed specification for either the input or output side of a stream.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StreamSpec {
    pub source: Option<String>,
    pub skip: Option<u64>,
    pub raw_unit: Option<u64>,
    pub offset: Option<u64>,
    pub unit_size: Option<usize>,
    pub pattern: Option<String>,
    pub gap: Option<u64>,
    pub pad_zeros: bool,
    pub overwrite: bool,
}

impl StreamSpec {
    pub fn from_framed(framed: FramedPattern) -> Self {
        let skip = framed.framing.skip;
        let raw_unit = framed.framing.raw_unit;
        let offset = framed.framing.offset;
        let unit_size = framed.framing.unit_size;
        let pattern = framed.raw_pattern;
        let gap = framed.framing.gap;
        Self {
            source: None,
            skip,
            raw_unit,
            offset,
            unit_size,
            pattern,
            gap,
            pad_zeros: false,
            overwrite: false,
        }
    }

    pub fn to_framed(&self) -> FramedPattern {
        FramedPattern {
            framing: crate::pattern::ContainerFraming {
                skip: self.skip,
                raw_unit: self.raw_unit,
                offset: self.offset,
                unit_size: self.unit_size,
                gap: self.gap,
            },
            fields: self
                .pattern
                .as_deref()
                .and_then(|p| crate::pattern::parse_input_pattern(p).ok())
                .unwrap_or_default(),
            raw_pattern: self.pattern.clone(),
        }
    }
}

/// Parsed stream I/O pattern combining optional source(s), input, inline manipulators, output, and optional sink.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StreamIoPattern {
    pub source: Option<String>,
    pub input: Option<StreamSpec>,
    pub sources: Vec<StreamSpec>,
    pub merge_specs: Vec<StreamSpec>,
    pub manipulators: Vec<String>,
    pub output: Option<StreamSpec>,
    pub sink: Option<String>,
    pub overwrite: bool,
}

/// Checks if a token represents a pipeline source keyword or file.
pub fn is_source(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    // Built-in source keywords
    if matches!(
        s,
        "stdin" | "zeros" | "ones" | "rand" | "random" | "counter" | "tuples"
    ) || s.starts_with("counter(")
    {
        return true;
    }
    // Explicit file() wrapper or quoted string
    if (s.starts_with("file(") && s.ends_with(')'))
        || (s.starts_with('\'') && s.ends_with('\''))
        || (s.starts_with('"') && s.ends_with('"'))
    {
        return true;
    }
    // Path-like (starts with ./, ../, /)
    if s.starts_with("./") || s.starts_with("../") || s.starts_with('/') {
        return true;
    }
    // Existing file on filesystem
    if std::path::Path::new(s).is_file() {
        return true;
    }
    // File with an alphanumeric extension that does not contain pattern/slicer delimiters
    if let Some(dot_idx) = s.rfind('.') {
        let ext = &s[dot_idx + 1..];
        if !ext.is_empty()
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
            && !s.contains(':')
            && !s.contains('[')
            && !s.contains(',')
            && !s.contains('*')
        {
            return true;
        }
    }
    false
}

/// Strips outer quotes or `file(...)` wrapper from a source specifier.
pub fn extract_source_name(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("file(") && s.ends_with(')') {
        s[5..s.len() - 1]
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_string()
    } else if (s.starts_with('\'') && s.ends_with('\'')) || (s.starts_with('"') && s.ends_with('"'))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Checks if a token represents a pipeline sink keyword or file.
pub fn is_sink(s: &str) -> bool {
    let s = s.trim();
    let norm = s.trim_end_matches("()");
    norm == "sum"
        || norm == "count"
        || norm == "avg"
        || norm == "mean"
        || norm == "min"
        || norm == "max"
        || norm == "entropy"
        || norm == "balance"
        || norm == "bit_balance"
        || norm == "variance"
        || norm == "stddev"
        || norm == "distinct"
        || norm == "unique"
        || norm == "stats"
        || norm == "profile"
        || s == "stdout"
        || s == "hex"
        || s == "bits"
        || s == "json"
        || s.starts_with("json:")
        || s == "csv"
        || s == "tuples"
        || s == "raw"
        || s == "bin"
        || s == "visual"
        || s == "integers"
        || (s.starts_with("file(") && s.ends_with(')'))
        || (s.starts_with('\'') && s.ends_with('\''))
        || (s.starts_with('"') && s.ends_with('"'))
}

/// Checks if a token represents an inline manipulator (e.g. `{0, 1|2}`, `xor(...)`, `add(...)`).
pub fn is_manipulator(s: &str) -> bool {
    let s = s.trim();
    (s.starts_with('{') && s.ends_with('}'))
        || s == "not"
        || s == "abs"
        || s == "sign"
        || s == "overwrite"
        || s.starts_with("overwrite(")
        || s.starts_with("interleave(")
        || s.starts_with("merge(")
        || s.starts_with("tee(")
        || s.starts_with("set(")
        || s.starts_with("set:")
        || s.starts_with("set=")
        || s.starts_with("xor(")
        || s.starts_with("and(")
        || s.starts_with("or(")
        || s.starts_with("add(")
        || s.starts_with("sub(")
        || s.starts_with("mul(")
        || s.starts_with("div(")
        || s.starts_with("mod(")
        || s.starts_with("clamp(")
        || s.starts_with("round(")
        || s.starts_with("filter(")
        || s.starts_with("rearrange(")
        || s.starts_with("glue(")
        || s.starts_with("concat(")
        || s.starts_with("split(")
        || s.starts_with("shift_left(")
        || s.starts_with("shift_right(")
        || s.starts_with("shift-left(")
        || s.starts_with("shift-right(")
        || s.starts_with("remove_right(")
        || s.starts_with("remove-right(")
}

fn is_pure_pattern(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty()
        || s.contains('[')
        || s.contains('+')
        || is_manipulator(s)
        || is_source(s)
        || is_sink(s)
    {
        return false;
    }
    if crate::preset::find_preset(s).is_some() {
        return true;
    }
    crate::pattern::parse_input_pattern(s).is_ok()
}

fn is_pure_slicer(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || is_manipulator(s) || is_source(s) || is_sink(s) {
        return false;
    }
    parse_size_with_suffix(s, "check", true).is_ok()
}

fn is_container_or_slicer(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() || is_manipulator(s) || is_source(s) || is_sink(s) || is_pure_pattern(s) {
        return false;
    }
    parse_stream_spec(s).is_ok()
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
    // Tuple projector
    if s.starts_with('{') && s.ends_with('}') {
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

/// Parse a stream specification that may include a source, e.g.:
/// `zeros:8`, `file('a.bin'):64:188B[11:13]+8`, `zeros:8, pad:zeros`, `188B[11:13]:overwrite`.
pub fn parse_stream_spec_with_source(s: &str) -> Result<StreamSpec, BddError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(StreamSpec::default());
    }

    // Check if inner arrow '->' exists: e.g. "zeros -> 8" or "file('a.bin') -> 16"
    if let Some(arrow_idx) = s.find("->") {
        let left = s[..arrow_idx].trim();
        let right = s[arrow_idx + 2..].trim();
        let mut left_spec = parse_stream_spec_with_source(left)?;
        let right_spec = parse_stream_spec_with_source(right)?;
        if left_spec.source.is_some() && right_spec.source.is_none() {
            left_spec.unit_size = right_spec.unit_size.or(left_spec.unit_size);
            left_spec.pattern = right_spec.pattern.or(left_spec.pattern);
            left_spec.raw_unit = right_spec.raw_unit.or(left_spec.raw_unit);
            left_spec.offset = right_spec.offset.or(left_spec.offset);
            left_spec.gap = right_spec.gap.or(left_spec.gap);
            left_spec.pad_zeros = left_spec.pad_zeros || right_spec.pad_zeros;
            left_spec.overwrite = left_spec.overwrite || right_spec.overwrite;
            return Ok(left_spec);
        }
    }

    let mut pad_zeros = false;
    let mut overwrite = false;

    // Check modifiers: ":pad:zeros", ":pad_zeros", ":overwrite", ":as_is", ":as-is", or ", pad:zeros"
    let mut clean_s = s.to_string();
    if clean_s.contains(":pad:zeros") {
        pad_zeros = true;
        clean_s = clean_s.replace(":pad:zeros", "");
    }
    if clean_s.contains(":pad_zeros") {
        pad_zeros = true;
        clean_s = clean_s.replace(":pad_zeros", "");
    }
    if clean_s.contains(", pad:zeros") {
        pad_zeros = true;
        clean_s = clean_s.replace(", pad:zeros", "");
    }
    if clean_s.contains(",pad:zeros") {
        pad_zeros = true;
        clean_s = clean_s.replace(",pad:zeros", "");
    }
    if clean_s.contains(":overwrite") {
        overwrite = true;
        clean_s = clean_s.replace(":overwrite", "");
    }
    if clean_s.contains(":as_is") || clean_s.contains(":as-is") {
        overwrite = true;
        clean_s = clean_s.replace(":as_is", "").replace(":as-is", "");
    }

    let s = clean_s.trim();

    // If pure source keyword
    if is_source(s) {
        return Ok(StreamSpec {
            source: Some(extract_source_name(s)),
            pad_zeros,
            overwrite,
            ..Default::default()
        });
    }

    // Split on colons outside brackets [] and parentheses ()
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut last_idx = 0;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            ':' if depth == 0 => {
                parts.push(s[last_idx..i].trim());
                last_idx = i + 1;
            }
            _ => {}
        }
    }
    parts.push(s[last_idx..].trim());

    if parts.len() == 1 {
        let mut spec = parse_stream_spec(parts[0])?;
        spec.pad_zeros = pad_zeros;
        spec.overwrite = overwrite;
        return Ok(spec);
    }

    let (source, start_idx) = if is_source(parts[0]) {
        (Some(extract_source_name(parts[0])), 1)
    } else {
        (None, 0)
    };

    let remaining_parts = &parts[start_idx..];
    if remaining_parts.is_empty() {
        return Ok(StreamSpec {
            source,
            pad_zeros,
            overwrite,
            ..Default::default()
        });
    }

    let remaining_s = remaining_parts.join(":");
    let mut spec = parse_stream_spec(&remaining_s)?;
    spec.source = source;
    spec.pad_zeros = pad_zeros;
    spec.overwrite = overwrite;
    Ok(spec)
}

/// Parse a full stream I/O pattern, e.g. "123:8[2:4]+8 -> 5B:8[2:4]", "8 -> xor(0xFF) -> 8",
/// "[ zeros:8, ones:8 ] -> hex", or unified pipeline "stdin -> 4U4U -> {0|1} -> 16U -> hex".
pub fn parse_stream_io_pattern(input_str: &str) -> Result<StreamIoPattern, BddError> {
    let trimmed = input_str.trim();
    if trimmed.is_empty() {
        return Ok(StreamIoPattern::default());
    }

    // Split on top-level '->' (outside any brackets [], parentheses (), or braces {})
    let mut depth = 0;
    let mut arrow_indices = Vec::new();
    let chars: Vec<(usize, char)> = trimmed.char_indices().collect();
    let n = chars.len();

    let mut i = 0;
    while i < n {
        let (byte_idx, c) = chars[i];
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            '-' if depth == 0 && i + 1 < n && chars[i + 1].1 == '>' => {
                arrow_indices.push(byte_idx);
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    let mut raw_segments: Vec<&str> = Vec::new();
    let mut prev_idx = 0;
    for &idx in &arrow_indices {
        let seg = trimmed[prev_idx..idx].trim();
        if !seg.is_empty() {
            raw_segments.push(seg);
        }
        prev_idx = idx + 2;
    }
    let last_seg = trimmed[prev_idx..].trim();
    if !last_seg.is_empty() {
        raw_segments.push(last_seg);
    }

    let mut sources = Vec::new();
    let mut merge_specs = Vec::new();
    let mut overwrite = false;

    // Check if segment 0 is a multi-source bracket: e.g. "[ zeros:8, ones:8 ]"
    if !raw_segments.is_empty()
        && raw_segments[0].starts_with('[')
        && raw_segments[0].ends_with(']')
    {
        let inner = &raw_segments[0][1..raw_segments[0].len() - 1].trim();
        let mut depth = 0;
        let mut has_comma = false;
        let mut parts = Vec::new();
        let mut last_idx = 0;
        for (idx, c) in inner.char_indices() {
            match c {
                '[' | '(' | '{' => depth += 1,
                ']' | ')' | '}' => {
                    if depth > 0 {
                        depth -= 1;
                    }
                }
                ',' if depth == 0 => {
                    has_comma = true;
                    parts.push(inner[last_idx..idx].trim());
                    last_idx = idx + 1;
                }
                _ => {}
            }
        }
        parts.push(inner[last_idx..].trim());

        if has_comma || (!parts.is_empty() && is_source(parts[0])) {
            for part in parts {
                if !part.is_empty() {
                    sources.push(parse_stream_spec_with_source(part)?);
                }
            }
            raw_segments.remove(0);
        }
    }

    let mut source = None;
    if sources.is_empty() && raw_segments.len() > 1 && is_source(raw_segments[0]) {
        source = Some(extract_source_name(raw_segments.remove(0)));
    }

    let mut sink = None;
    if (source.is_some() || !sources.is_empty() || raw_segments.len() > 1)
        && !raw_segments.is_empty()
        && is_sink(raw_segments.last().unwrap())
    {
        sink = Some(extract_source_name(raw_segments.pop().unwrap()));
    }

    // Filter and process intermediate segments for overwrite, interleave, and merge
    let mut segments: Vec<&str> = Vec::new();
    for seg in raw_segments {
        let s_trim = seg.trim();
        if s_trim == "overwrite" {
            overwrite = true;
        } else if s_trim.starts_with("overwrite(") && s_trim.ends_with(')') {
            overwrite = true;
            let inner = &s_trim[10..s_trim.len() - 1].trim();
            if !inner.is_empty() {
                segments.push(inner);
            }
        } else if s_trim.starts_with("interleave(") && s_trim.ends_with(')') {
            let inner = &s_trim[11..s_trim.len() - 1].trim();
            merge_specs.push(parse_stream_spec_with_source(inner)?);
        } else if s_trim.starts_with("merge(") && s_trim.ends_with(')') {
            let inner = &s_trim[6..s_trim.len() - 1].trim();
            merge_specs.push(parse_stream_spec_with_source(inner)?);
        } else {
            segments.push(seg);
        }
    }

    let mut input = None;
    let mut output = None;
    let mut manipulators = Vec::new();

    if !sources.is_empty() {
        source = sources[0].source.clone();
        input = Some(sources[0].clone());
        if sources[0].overwrite {
            overwrite = true;
        }
        for s in sources.iter().skip(1) {
            merge_specs.push(s.clone());
        }
    }

    if segments.is_empty() {
        if output.is_none() {
            let is_formatted_sink = match sink.as_deref() {
                Some("hex" | "bits" | "json" | "integers" | "csv" | "visual") => true,
                Some(s) if s.starts_with("json:") => true,
                _ => false,
            };
            if is_formatted_sink || !sources.is_empty() {
                if let Some(ref inp) = input {
                    if let Some(u) = inp.unit_size {
                        output = Some(StreamSpec {
                            unit_size: Some(u),
                            ..Default::default()
                        });
                    }
                }
            }
        }
        return Ok(StreamIoPattern {
            source,
            input,
            sources,
            merge_specs,
            manipulators: Vec::new(),
            output,
            sink,
            overwrite,
        });
    }

    // Check if segment 0 and 1 form an explicit (slicer/container -> pattern) pair:
    // e.g. "8 -> 4U4U" or "20B[8:8] -> 8U"
    if segments.len() >= 2 && is_container_or_slicer(segments[0]) && is_pure_pattern(segments[1]) {
        let u_str = segments.remove(0);
        let p_str = segments.remove(0);
        let mut in_spec = parse_stream_spec_with_source(u_str)?;
        in_spec.pattern = Some(p_str.to_string());
        if let Ok(unpacker) = crate::pattern::TupleUnpacker::new(p_str) {
            if let Some(u) = in_spec.unit_size {
                if unpacker.total_bits != u {
                    return Err(BddError::CliError(format!(
                        "Dimension mismatch in stream pattern: Slicer specifies {} bits, but pattern '{}' requires {} bits",
                        u, p_str, unpacker.total_bits
                    )));
                }
            } else {
                in_spec.unit_size = Some(unpacker.total_bits);
            }
        }
        if in_spec.source.is_some() && source.is_none() {
            source = in_spec.source.clone();
        }
        if in_spec.overwrite {
            overwrite = true;
        }
        input = Some(in_spec);
    } else if !segments.is_empty() && !is_manipulator(segments[0]) {
        if input.is_none() {
            let in_spec = parse_stream_spec_with_source(segments.remove(0))?;
            if in_spec.source.is_some() && source.is_none() {
                source = in_spec.source.clone();
            }
            if in_spec.overwrite {
                overwrite = true;
            }
            input = Some(in_spec);
        } else {
            // Already have input from multi-source bracket (e.g. [zeros, ones] -> 8)
            if is_pure_slicer(segments[0]) {
                let u_val = parse_size_with_suffix(segments.remove(0), "unit size", true)? as usize;
                if let Some(ref mut inp) = input {
                    if inp.unit_size.is_none() {
                        inp.unit_size = Some(u_val);
                    }
                }
                for ms in &mut merge_specs {
                    if ms.unit_size.is_none() {
                        ms.unit_size = Some(u_val);
                    }
                }
                for s in &mut sources {
                    if s.unit_size.is_none() {
                        s.unit_size = Some(u_val);
                    }
                }
                if output.is_none() && (segments.is_empty() || is_manipulator(segments[0])) {
                    output = Some(StreamSpec {
                        unit_size: Some(u_val),
                        ..Default::default()
                    });
                }
            } else {
                segments.remove(0);
            }
        }
    }

    // Check if the last two segments form an explicit (pattern -> slicer/container) pair:
    // e.g. "16U -> 16" or "8U -> 20B[8:8]"
    if segments.len() >= 2
        && is_pure_pattern(segments[segments.len() - 2])
        && is_container_or_slicer(segments.last().unwrap())
    {
        let u_str = segments.pop().unwrap();
        let p_str = segments.pop().unwrap();
        let mut out_spec = parse_stream_spec(u_str)?;
        out_spec.pattern = Some(p_str.to_string());
        if let Ok(unpacker) = crate::pattern::TupleUnpacker::new(p_str) {
            if let Some(u) = out_spec.unit_size {
                if unpacker.total_bits != u {
                    return Err(BddError::CliError(format!(
                        "Dimension mismatch in stream pattern: Pattern '{}' requires {} bits, but slicer specifies {} bits",
                        p_str, unpacker.total_bits, u
                    )));
                }
            } else {
                out_spec.unit_size = Some(unpacker.total_bits);
            }
        }
        if out_spec.overwrite {
            overwrite = true;
        }
        output = Some(out_spec);
    } else if !segments.is_empty() && !is_manipulator(segments.last().unwrap()) {
        let out_spec = parse_stream_spec(segments.pop().unwrap())?;
        if out_spec.overwrite {
            overwrite = true;
        }
        output = Some(out_spec);
    }

    // Anything remaining in segments is an inline manipulator
    for seg in segments {
        manipulators.push(seg.to_string());
    }

    // When output framing is omitted, inherit unit size from input for multi-source
    // streams or formatted terminal/data sinks (hex, bits, json, etc.)
    if output.is_none() {
        let is_formatted_sink = match sink.as_deref() {
            Some("hex" | "bits" | "json" | "integers" | "csv" | "visual") => true,
            Some(s) if s.starts_with("json:") => true,
            _ => false,
        };
        if is_formatted_sink || !sources.is_empty() {
            if let Some(ref inp) = input {
                if let Some(u) = inp.unit_size {
                    output = Some(StreamSpec {
                        unit_size: Some(u),
                        ..Default::default()
                    });
                }
            }
        }
    }

    Ok(StreamIoPattern {
        source,
        input,
        sources,
        merge_specs,
        manipulators,
        output,
        sink,
        overwrite,
    })
}

/// Parse a single stream specification (input or output).
pub fn parse_stream_spec(s: &str) -> Result<StreamSpec, BddError> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(StreamSpec::default());
    }
    let framed = FramedPattern::parse(s)?;
    Ok(StreamSpec::from_framed(framed))
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

    #[test]
    fn test_unified_stream_pipeline_parsing() {
        // "4U4U -> {0|1} -> 16U"
        let p1 = parse_stream_io_pattern("4U4U -> {0|1} -> 16U").unwrap();
        assert_eq!(p1.input.unwrap().pattern, Some("4U4U".to_string()));
        assert_eq!(p1.manipulators, vec!["{0|1}"]);
        assert_eq!(p1.output.unwrap().pattern, Some("16U".to_string()));
        assert_eq!(p1.source, None);
        assert_eq!(p1.sink, None);

        // "4U4U -> {0|1} -> 16U -> hex"
        let p2 = parse_stream_io_pattern("4U4U -> {0|1} -> 16U -> hex").unwrap();
        assert_eq!(p2.input.unwrap().pattern, Some("4U4U".to_string()));
        assert_eq!(p2.manipulators, vec!["{0|1}"]);
        assert_eq!(p2.output.unwrap().pattern, Some("16U".to_string()));
        assert_eq!(p2.sink, Some("hex".to_string()));

        // "stdin -> 8 -> 4U4U -> {0|1} -> 16U -> 16 -> stdout"
        let p3 =
            parse_stream_io_pattern("stdin -> 8 -> 4U4U -> {0|1} -> 16U -> 16 -> stdout").unwrap();
        assert_eq!(p3.source, Some("stdin".to_string()));
        let inp3 = p3.input.unwrap();
        assert_eq!(inp3.unit_size, Some(8));
        assert_eq!(inp3.pattern, Some("4U4U".to_string()));
        assert_eq!(p3.manipulators, vec!["{0|1}"]);
        let out3 = p3.output.unwrap();
        assert_eq!(out3.unit_size, Some(16));
        assert_eq!(out3.pattern, Some("16U".to_string()));
        assert_eq!(p3.sink, Some("stdout".to_string()));

        // "zeros -> 8 -> xor(0xAA) -> hex"
        let p4 = parse_stream_io_pattern("zeros -> 8 -> xor(0xAA) -> hex").unwrap();
        assert_eq!(p4.source, Some("zeros".to_string()));
        assert_eq!(p4.input.unwrap().unit_size, Some(8));
        assert_eq!(p4.manipulators, vec!["xor(0xAA)"]);
        assert_eq!(p4.sink, Some("hex".to_string()));

        // Dimension mismatch error: "12 -> 4U4U -> hex"
        let err = parse_stream_io_pattern("12 -> 4U4U -> hex");
        assert!(err.is_err());
    }

    #[test]
    fn test_multi_source_brackets_and_interleave() {
        // Multi-source bracket: "[ zeros:8, ones:8 ] -> hex"
        let p1 = parse_stream_io_pattern("[ zeros:8, ones:8 ] -> hex").unwrap();
        assert_eq!(p1.sources.len(), 2);
        assert_eq!(p1.sources[0].source, Some("zeros".to_string()));
        assert_eq!(p1.sources[0].unit_size, Some(8));
        assert_eq!(p1.sources[1].source, Some("ones".to_string()));
        assert_eq!(p1.sources[1].unit_size, Some(8));
        assert_eq!(p1.merge_specs.len(), 1);
        assert_eq!(p1.merge_specs[0].source, Some("ones".to_string()));
        assert_eq!(p1.sink, Some("hex".to_string()));

        // Multi-source with downstream slicer: "[ zeros, counter ] -> 16 -> hex"
        let p2 = parse_stream_io_pattern("[ zeros, counter ] -> 16 -> hex").unwrap();
        assert_eq!(p2.sources.len(), 2);
        assert_eq!(p2.sources[0].source, Some("zeros".to_string()));
        assert_eq!(p2.sources[0].unit_size, Some(16));
        assert_eq!(p2.sources[1].source, Some("counter".to_string()));
        assert_eq!(p2.sources[1].unit_size, Some(16));
        assert_eq!(p2.merge_specs[0].unit_size, Some(16));
        assert_eq!(p2.sink, Some("hex".to_string()));

        // Pipe interleave: "stdin:16 -> interleave(zeros:8) -> hex"
        let p3 = parse_stream_io_pattern("stdin:16 -> interleave(zeros:8) -> hex").unwrap();
        assert_eq!(p3.source, Some("stdin".to_string()));
        assert_eq!(p3.input.unwrap().unit_size, Some(16));
        assert_eq!(p3.merge_specs.len(), 1);
        assert_eq!(p3.merge_specs[0].source, Some("zeros".to_string()));
        assert_eq!(p3.merge_specs[0].unit_size, Some(8));
        assert_eq!(p3.sink, Some("hex".to_string()));

        // Pipe interleave with pad:zeros: "stdin:16 -> interleave(file('tags.bin'):8, pad:zeros) -> stdout"
        let p4 = parse_stream_io_pattern(
            "stdin:16 -> interleave(file('tags.bin'):8, pad:zeros) -> stdout",
        )
        .unwrap();
        assert_eq!(p4.merge_specs.len(), 1);
        assert_eq!(p4.merge_specs[0].source, Some("tags.bin".to_string()));
        assert_eq!(p4.merge_specs[0].unit_size, Some(8));
        assert!(p4.merge_specs[0].pad_zeros);

        // Overwrite mode: "stream.ts -> 188B[11:13] -> xor(0x1FFF) -> overwrite -> stdout"
        let p5 = parse_stream_io_pattern(
            "file('stream.ts') -> 188B[11:13] -> xor(0x1FFF) -> overwrite -> stdout",
        )
        .unwrap();
        assert_eq!(p5.source, Some("stream.ts".to_string()));
        let inp5 = p5.input.as_ref().unwrap();
        assert_eq!(inp5.raw_unit, Some(1504));
        assert_eq!(inp5.offset, Some(11));
        assert_eq!(inp5.unit_size, Some(13));
        assert_eq!(p5.manipulators, vec!["xor(0x1FFF)"]);
        assert!(p5.overwrite);
    }
}
