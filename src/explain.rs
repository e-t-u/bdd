//! Schema introspection and bit layout explainer for bdd patterns.
//!
//! Provides detailed bit offset calculations, byte boundaries, and field semantics
//! allowing AI models and developers to verify patterns before processing.

use crate::error::BddError;
use crate::pattern::parse_input_pattern;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PatternExplanation {
    pub pattern: String,
    pub total_bits: usize,
    pub total_bytes: f64,
    pub is_byte_aligned: bool,
    pub field_count: usize,
    pub fields: Vec<FieldExplanation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FieldExplanation {
    pub index: usize,
    pub name: Option<String>,
    pub char_code: char,
    pub type_name: &'static str,
    pub bits: usize,
    pub bit_start: usize,
    pub bit_end: usize,
    pub byte_range: String,
}

fn type_code_description(c: char) -> &'static str {
    match c {
        'U' => "Unsigned Integer (Big-Endian bits)",
        'u' => "Unsigned Integer (Little-Endian bits)",
        'S' => "Signed Integer (Big-Endian bits)",
        's' => "Signed Integer (Little-Endian bits)",
        'B' => "Binary bit string (Big-Endian bits)",
        'b' => "Binary bit string (Little-Endian bits)",
        'M' => "Magnitude + Sign bit (Big-Endian bits)",
        'm' => "Magnitude + Sign bit (Little-Endian bits)",
        'F' => "32-bit IEEE 754 Float (Big-Endian bits)",
        'f' => "32-bit IEEE 754 Float (Little-Endian bits)",
        'D' => "64-bit IEEE 754 Double (Big-Endian bits)",
        'd' => "64-bit IEEE 754 Double (Little-Endian bits)",
        'H' => "16-bit IEEE 754 Half-Precision Float (Big-Endian bits)",
        'h' => "16-bit IEEE 754 Half-Precision Float (Little-Endian bits)",
        'Y' => "16-bit Bfloat16 Float (Big-Endian bits)",
        'y' => "16-bit Bfloat16 Float (Little-Endian bits)",
        'E' => "OCP FP8 (E4M3) / FP6 / FP4 Float (Big-Endian bits)",
        'e' => "OCP FP8 (E4M3) / FP6 / FP4 Float (Little-Endian bits)",
        'Q' => "OCP FP8 (E5M2) Float (Big-Endian bits)",
        'q' => "OCP FP8 (E5M2) Float (Little-Endian bits)",
        'C' => "ASCII / Character bytes (Big-Endian bits)",
        'c' => "ASCII / Character bytes (Little-Endian bits)",
        'x' | 'X' => "Skip / Ignored padding bits",
        'K' => "Auto-incrementing Counter (Big-Endian bits)",
        'k' => "Auto-incrementing Counter (Little-Endian bits)",
        'V' => "Raw Bit Vector (Big-Endian bits)",
        'v' => "Raw Bit Vector (Little-Endian bits)",
        _ => "Unknown",
    }
}

/// Computes the structured explanation of a pattern.
pub fn explain_pattern(pattern_str: &str) -> Result<PatternExplanation, BddError> {
    let items = parse_input_pattern(pattern_str)?;
    let total_bits: usize = items.iter().map(|p| p.bits).sum();
    let total_bytes = total_bits as f64 / 8.0;
    let is_byte_aligned = total_bits.is_multiple_of(8);

    let mut fields = Vec::new();
    let mut current_bit = 0;

    for (idx, item) in items.iter().enumerate() {
        let bit_start = current_bit;
        let bit_end = if item.bits > 0 {
            current_bit + item.bits - 1
        } else {
            current_bit
        };
        let start_byte = bit_start / 8;
        let end_byte = bit_end / 8;

        let byte_range = if start_byte == end_byte {
            format!("byte {}", start_byte)
        } else {
            format!("byte {}..{}", start_byte, end_byte)
        };

        fields.push(FieldExplanation {
            index: idx,
            name: item.name.clone(),
            char_code: item.char_code,
            type_name: type_code_description(item.char_code),
            bits: item.bits,
            bit_start,
            bit_end,
            byte_range,
        });

        current_bit += item.bits;
    }

    Ok(PatternExplanation {
        pattern: pattern_str.to_string(),
        total_bits,
        total_bytes,
        is_byte_aligned,
        field_count: fields.len(),
        fields,
    })
}

/// Formats the pattern explanation as a clean, aligned human-readable table.
pub fn format_explanation_text(exp: &PatternExplanation) -> String {
    let mut out = String::new();
    let alignment_str = if exp.is_byte_aligned {
        "Byte-aligned"
    } else {
        "Unaligned (sub-byte remainder)"
    };
    out.push_str(&format!("Pattern:     {}\n", exp.pattern));
    out.push_str(&format!(
        "Total Width: {} bits ({:.3} bytes) [{}]\n",
        exp.total_bits, exp.total_bytes, alignment_str
    ));
    out.push_str(&format!("Fields:      {}\n\n", exp.field_count));

    out.push_str(&format!(
        "{:<3}  {:<16}  {:<6}  {:<12}  {:<12}  {}\n",
        "#", "NAME", "BITS", "BIT RANGE", "BYTE RANGE", "FIELD TYPE"
    ));
    out.push_str(&format!(
        "{:-<3}  {:-<16}  {:-<6}  {:-<12}  {:-<12}  {:-<30}\n",
        "", "", "", "", "", ""
    ));

    for f in &exp.fields {
        let name_display = f.name.as_deref().unwrap_or("-");
        let bit_range = format!("[{}..{}]", f.bit_start, f.bit_end);
        let bits_str = format!("{}{}", f.bits, f.char_code);
        out.push_str(&format!(
            "{:<3}  {:<16}  {:<6}  {:<12}  {:<12}  {}\n",
            f.index, name_display, bits_str, bit_range, f.byte_range, f.type_name
        ));
    }
    out
}

/// Formats the pattern explanation as JSON.
pub fn format_explanation_json(exp: &PatternExplanation) -> String {
    serde_json::to_string_pretty(exp).unwrap_or_else(|_| "{}".to_string())
}

fn sanitize_ident(name: Option<&str>, idx: usize, is_rust: bool) -> String {
    let raw = match name {
        Some(s) if !s.trim().is_empty() => s.trim(),
        _ => return format!("field_{}", idx),
    };
    let mut cleaned = String::new();
    for (i, c) in raw.chars().enumerate() {
        if c.is_ascii_alphanumeric() || c == '_' {
            if i == 0 && c.is_ascii_digit() {
                cleaned.push('_');
            }
            cleaned.push(c.to_ascii_lowercase());
        } else {
            cleaned.push('_');
        }
    }
    if cleaned.is_empty() {
        return format!("field_{}", idx);
    }
    if is_rust {
        const RUST_KEYWORDS: &[&str] = &[
            "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
            "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
            "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true",
            "type", "unsafe", "use", "where", "while", "async", "await", "dyn",
        ];
        if RUST_KEYWORDS.contains(&cleaned.as_str()) {
            return format!("r#{}", cleaned);
        }
    } else {
        const C_KEYWORDS: &[&str] = &[
            "auto", "break", "case", "char", "const", "continue", "default", "do", "double",
            "else", "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long",
            "register", "restrict", "return", "short", "signed", "sizeof", "static", "struct",
            "switch", "typedef", "union", "unsigned", "void", "volatile", "while",
        ];
        if C_KEYWORDS.contains(&cleaned.as_str()) {
            return format!("_{}", cleaned);
        }
    }
    cleaned
}

/// Generates a copy-pasteable packed C struct header definition from a pattern.
pub fn generate_c_struct(pattern_str: &str, struct_name: Option<&str>) -> Result<String, BddError> {
    let exp = explain_pattern(pattern_str)?;
    let name = struct_name.unwrap_or("BddPacket");

    let mut out = String::new();
    out.push_str("/* Generated by bdd - Binary Data Definition bitstream engine */\n");
    out.push_str(&format!(
        "/* Pattern: {} ({} bits, {:.3} bytes) */\n\n",
        exp.pattern, exp.total_bits, exp.total_bytes
    ));
    out.push_str("#ifndef BDD_GENERATED_STRUCT_H\n#define BDD_GENERATED_STRUCT_H\n\n");
    out.push_str("#include <stdint.h>\n#include <stdbool.h>\n\n");
    out.push_str("#pragma pack(push, 1)\n");
    out.push_str("typedef struct {\n");

    for f in &exp.fields {
        let field_ident = sanitize_ident(f.name.as_deref(), f.index, false);
        let bit_info = format!(
            "/* bits [{}..{}] ({} bits, {}) - {} */",
            f.bit_start, f.bit_end, f.bits, f.byte_range, f.type_name
        );

        let type_decl = match f.char_code {
            'C' | 'c' if f.bits % 8 == 0 && f.bits > 8 => {
                format!("char {}[{}]", field_ident, f.bits / 8)
            }
            'C' | 'c' if f.bits == 8 => format!("char {}", field_ident),
            'F' | 'f' if f.bits == 32 => format!("float {}", field_ident),
            'D' | 'd' if f.bits == 64 => format!("double {}", field_ident),
            'S' | 's' => match f.bits {
                8 => format!("int8_t {}", field_ident),
                16 => format!("int16_t {}", field_ident),
                32 => format!("int32_t {}", field_ident),
                64 => format!("int64_t {}", field_ident),
                b if b <= 8 => format!("int8_t {} : {}", field_ident, b),
                b if b <= 16 => format!("int16_t {} : {}", field_ident, b),
                b if b <= 32 => format!("int32_t {} : {}", field_ident, b),
                b if b <= 64 => format!("int64_t {} : {}", field_ident, b),
                b if b % 8 == 0 => format!("int8_t {}[{}]", field_ident, b / 8),
                b => format!("int8_t {}[{}]", field_ident, b.div_ceil(8)),
            },
            _ => match f.bits {
                8 => format!("uint8_t {}", field_ident),
                16 => format!("uint16_t {}", field_ident),
                32 => format!("uint32_t {}", field_ident),
                64 => format!("uint64_t {}", field_ident),
                b if b <= 8 => format!("uint8_t {} : {}", field_ident, b),
                b if b <= 16 => format!("uint16_t {} : {}", field_ident, b),
                b if b <= 32 => format!("uint32_t {} : {}", field_ident, b),
                b if b <= 64 => format!("uint64_t {} : {}", field_ident, b),
                b if b % 8 == 0 => format!("uint8_t {}[{}]", field_ident, b / 8),
                b => format!("uint8_t {}[{}]", field_ident, b.div_ceil(8)),
            },
        };
        let decl_with_semi = format!("{};", type_decl);
        out.push_str(&format!("    {:<28} {}\n", decl_with_semi, bit_info));
    }

    out.push_str(&format!("}} {};\n", name));
    out.push_str("#pragma pack(pop)\n\n");
    out.push_str("#endif /* BDD_GENERATED_STRUCT_H */\n");

    Ok(out)
}

/// Generates a copy-pasteable packed Rust struct definition from a pattern.
pub fn generate_rust_struct(
    pattern_str: &str,
    struct_name: Option<&str>,
) -> Result<String, BddError> {
    let exp = explain_pattern(pattern_str)?;
    let name = struct_name.unwrap_or("BddPacket");

    let mut out = String::new();
    out.push_str("// Generated by bdd - Binary Data Definition bitstream engine\n");
    out.push_str(&format!(
        "// Pattern: {} ({} bits, {:.3} bytes)\n\n",
        exp.pattern, exp.total_bits, exp.total_bytes
    ));
    out.push_str("#[repr(C, packed)]\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq)]\n");
    out.push_str(&format!("pub struct {} {{\n", name));

    for f in &exp.fields {
        let field_ident = sanitize_ident(f.name.as_deref(), f.index, true);
        let bit_info = format!(
            "// bits [{}..{}] ({} bits, {}) - {}",
            f.bit_start, f.bit_end, f.bits, f.byte_range, f.type_name
        );

        let type_decl = match f.char_code {
            'F' | 'f' if f.bits == 32 => "f32".to_string(),
            'D' | 'd' if f.bits == 64 => "f64".to_string(),
            'S' | 's' => match f.bits {
                8 => "i8".to_string(),
                16 => "i16".to_string(),
                32 => "i32".to_string(),
                64 => "i64".to_string(),
                b if b <= 8 => "i8".to_string(),
                b if b <= 16 => "i16".to_string(),
                b if b <= 32 => "i32".to_string(),
                b if b <= 64 => "i64".to_string(),
                b => format!("[i8; {}]", b.div_ceil(8)),
            },
            _ => match f.bits {
                8 => "u8".to_string(),
                16 => "u16".to_string(),
                32 => "u32".to_string(),
                64 => "u64".to_string(),
                b if b <= 8 => "u8".to_string(),
                b if b <= 16 => "u16".to_string(),
                b if b <= 32 => "u32".to_string(),
                b if b <= 64 => "u64".to_string(),
                b => format!("[u8; {}]", b.div_ceil(8)),
            },
        };

        out.push_str(&format!(
            "    pub {}: {:<10} {}\n",
            field_ident,
            type_decl + ",",
            bit_info
        ));
    }

    out.push_str("}\n");

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_c_and_rust_struct() {
        let pat = "sync:8u,te:1b,pusi:1b,pid:13u,payload:1472U";
        let c_code = generate_c_struct(pat, Some("MpegTsHeader")).unwrap();
        assert!(c_code.contains("typedef struct {"));
        assert!(c_code.contains("uint8_t sync;"));
        assert!(c_code.contains("uint8_t te : 1;"));
        assert!(c_code.contains("uint16_t pid : 13;"));
        assert!(c_code.contains("uint8_t payload[184];"));
        assert!(c_code.contains("} MpegTsHeader;"));

        let rust_code = generate_rust_struct(pat, Some("MpegTsHeader")).unwrap();
        assert!(rust_code.contains("pub struct MpegTsHeader {"));
        assert!(rust_code.contains("pub sync: u8,"));
        assert!(rust_code.contains("pub te: u8,"));
        assert!(rust_code.contains("pub pid: u16,"));
        assert!(rust_code.contains("pub payload: [u8; 184],"));
    }
}
