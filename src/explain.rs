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
