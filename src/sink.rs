use crate::field::{reverse_bits, Field};
use num_bigint::BigUint;
use num_traits::{One, ToPrimitive, Zero};
use std::io::Write;

/// Sink interface for writing units of a given bit width.
pub trait UnitSink {
    fn write_bits(&mut self, unit: BigUint, bits: usize) -> std::io::Result<()>;
    fn flush_stream(&mut self) -> std::io::Result<()>;
}

/// Bit-packing sink that outputs packed bytes into an underlying writer.
pub struct FileOutputStream<W> {
    writer: W,
    buffer: BigUint,
    bits_in_buffer: usize,
    reverse_bytes: bool,
    reverse_unit: bool,
}

impl<W: Write> FileOutputStream<W> {
    pub fn new(writer: W, reverse_bytes: bool, reverse_unit: bool) -> Self {
        Self {
            writer,
            buffer: BigUint::zero(),
            bits_in_buffer: 0,
            reverse_bytes,
            reverse_unit,
        }
    }
}

impl<W: Write> UnitSink for FileOutputStream<W> {
    fn write_bits(&mut self, mut unit: BigUint, bits: usize) -> std::io::Result<()> {
        if self.reverse_unit {
            unit = reverse_bits(&unit, bits);
        }
        let mask = if bits > 0 {
            (BigUint::one() << bits) - 1u32
        } else {
            BigUint::zero()
        };
        unit &= mask;
        self.buffer = (std::mem::take(&mut self.buffer) << bits) | unit;
        self.bits_in_buffer += bits;

        while self.bits_in_buffer >= 8 {
            let right_edge = self.bits_in_buffer - 8;
            let mut val = (&self.buffer >> right_edge).to_u8().unwrap_or(0);
            if self.reverse_bytes {
                val = val.reverse_bits();
            }
            self.writer.write_all(&[val])?;
            self.bits_in_buffer -= 8;
            let rem_mask = if self.bits_in_buffer > 0 {
                (BigUint::one() << self.bits_in_buffer) - 1u32
            } else {
                BigUint::zero()
            };
            self.buffer &= rem_mask;
        }
        Ok(())
    }

    fn flush_stream(&mut self) -> std::io::Result<()> {
        if self.bits_in_buffer > 0 {
            let shift = 8 - self.bits_in_buffer;
            let mut val = (std::mem::take(&mut self.buffer) << shift)
                .to_u8()
                .unwrap_or(0);
            if self.reverse_bytes {
                val = val.reverse_bits();
            }
            self.writer.write_all(&[val])?;
            self.bits_in_buffer = 0;
        }
        self.writer.flush()
    }
}

/// Sink formatting units as zero-padded hexadecimal words.
pub struct HexOutputStream<W> {
    writer: W,
    output_column: usize,
    output_units_in_line: usize,
}

impl<W: Write> HexOutputStream<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            output_column: 0,
            output_units_in_line: 0,
        }
    }
}

impl<W: Write> UnitSink for HexOutputStream<W> {
    fn write_bits(&mut self, mut unit: BigUint, bits: usize) -> std::io::Result<()> {
        let mask = if bits > 0 {
            (BigUint::one() << bits) - 1u32
        } else {
            BigUint::zero()
        };
        unit &= mask;
        let hex_digits = if bits == 0 { 1 } else { ((bits - 1) / 4) + 1 };
        let h = format!("{:x}", unit);
        let filler = "0".repeat(hex_digits.saturating_sub(h.len()));

        if self.output_column > 40
            && self.output_units_in_line > 0
            && self.output_units_in_line.is_power_of_two()
        {
            self.writer.write_all(b"\n")?;
            self.output_column = 0;
            self.output_units_in_line = 0;
        }

        if self.output_units_in_line > 0 {
            self.writer.write_all(b" ")?;
            self.output_column += 1;
        }

        let s = format!("{}{h}", filler);
        self.writer.write_all(s.as_bytes())?;
        self.output_column += hex_digits;
        self.output_units_in_line += 1;
        Ok(())
    }

    fn flush_stream(&mut self) -> std::io::Result<()> {
        if self.output_column > 0 {
            self.writer.write_all(b"\n")?;
        }
        self.writer.flush()
    }
}

/// Sink formatting units as binary strings.
pub struct BitOutputStream<W> {
    writer: W,
    output_column: usize,
    output_units_in_line: usize,
}

impl<W: Write> BitOutputStream<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            output_column: 0,
            output_units_in_line: 0,
        }
    }
}

impl<W: Write> UnitSink for BitOutputStream<W> {
    fn write_bits(&mut self, mut unit: BigUint, bits: usize) -> std::io::Result<()> {
        let mask = if bits > 0 {
            (BigUint::one() << bits) - 1u32
        } else {
            BigUint::zero()
        };
        unit &= mask;
        let b_str = format!("{:b}", unit);
        let filler = "0".repeat(bits.saturating_sub(b_str.len()));

        if self.output_column > 40
            && self.output_units_in_line > 0
            && self.output_units_in_line.is_power_of_two()
        {
            self.writer.write_all(b"\n")?;
            self.output_column = 0;
            self.output_units_in_line = 0;
        }

        if self.output_units_in_line > 0 {
            self.writer.write_all(b" ")?;
            self.output_column += 1;
        }

        let s = format!("{}{b_str}", filler);
        self.writer.write_all(s.as_bytes())?;
        self.output_column += bits;
        self.output_units_in_line += 1;
        Ok(())
    }

    fn flush_stream(&mut self) -> std::io::Result<()> {
        if self.output_column > 0 {
            self.writer.write_all(b"\n")?;
        }
        self.writer.flush()
    }
}

/// Sink formatting units as decimal integers, one per line.
pub struct IntegerOutputStream<W> {
    writer: W,
}

impl<W: Write> IntegerOutputStream<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> UnitSink for IntegerOutputStream<W> {
    fn write_bits(&mut self, mut unit: BigUint, bits: usize) -> std::io::Result<()> {
        let mask = if bits > 0 {
            (BigUint::one() << bits) - 1u32
        } else {
            BigUint::zero()
        };
        unit &= mask;
        writeln!(self.writer, "{}", unit)?;
        Ok(())
    }

    fn flush_stream(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// Direct tuple sink printing CSV-delimited lines.
/// Generic sink interface for receiving unpacked/processed tuples.
pub trait TupleSink {
    fn write_tuple(&mut self, tuple: &[Field]) -> std::io::Result<()>;
    fn flush_stream(&mut self) -> std::io::Result<()>;
}

pub struct TupleDirectOutput<W> {
    writer: W,
}

impl<W: Write> TupleDirectOutput<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> TupleSink for TupleDirectOutput<W> {
    fn write_tuple(&mut self, tuple: &[Field]) -> std::io::Result<()> {
        let str_list: Vec<String> = tuple.iter().map(|f| f.to_string()).collect();
        writeln!(self.writer, "{}", str_list.join(","))?;
        Ok(())
    }

    fn flush_stream(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// Sink formatting tuples as NDJSON records.
pub struct JsonOutputStream<W> {
    writer: W,
    field_names: Option<Vec<String>>,
}

impl<W: Write> JsonOutputStream<W> {
    pub fn new(writer: W, field_names: Option<Vec<String>>) -> Self {
        Self {
            writer,
            field_names,
        }
    }
}

impl<W: Write> TupleSink for JsonOutputStream<W> {
    fn write_tuple(&mut self, tuple: &[Field]) -> std::io::Result<()> {
        if let Some(ref names) = self.field_names {
            let mut map = serde_json::Map::new();
            for (i, f) in tuple.iter().enumerate() {
                let key = names
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("field_{}", i));
                let val = match f {
                    Field::UInt(u) => {
                        if let Some(n) = u.to_u64() {
                            serde_json::Value::Number(n.into())
                        } else {
                            serde_json::Value::String(u.to_string())
                        }
                    }
                    Field::Int(i) => {
                        if let Some(n) = i.to_i64() {
                            serde_json::Value::Number(n.into())
                        } else {
                            serde_json::Value::String(i.to_string())
                        }
                    }
                    Field::Float(fl) => {
                        if fl.is_nan() || fl.is_infinite() {
                            serde_json::Value::Null
                        } else if let Some(n) = serde_json::Number::from_f64(*fl) {
                            serde_json::Value::Number(n)
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    Field::Bytes(b) => {
                        let s = String::from_utf8_lossy(b).into_owned();
                        serde_json::Value::String(s)
                    }
                };
                map.insert(key, val);
            }
            writeln!(
                self.writer,
                "{}",
                serde_json::to_string(&serde_json::Value::Object(map))?
            )?;
        } else {
            let items: Vec<String> = tuple
                .iter()
                .map(|f| match f {
                    Field::UInt(u) => u.to_string(),
                    Field::Int(i) => i.to_string(),
                    Field::Float(fl) => {
                        if fl.is_nan() {
                            "null".to_string()
                        } else if fl.is_infinite() {
                            if fl.is_sign_negative() {
                                "-1e999".to_string()
                            } else {
                                "1e999".to_string()
                            }
                        } else {
                            format!("{}", fl)
                        }
                    }
                    Field::Bytes(b) => {
                        let s = String::from_utf8_lossy(b);
                        format!("\"{}\"", s.escape_default())
                    }
                })
                .collect();
            writeln!(self.writer, "[{}]", items.join(","))?;
        }
        Ok(())
    }

    fn flush_stream(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

/// Sink formatting tuples as CSV rows with optional header.
pub struct CsvOutputStream<W> {
    writer: W,
    header: Option<String>,
    header_written: bool,
}

impl<W: Write> CsvOutputStream<W> {
    pub fn new(writer: W, header: Option<String>) -> Self {
        Self {
            writer,
            header,
            header_written: false,
        }
    }
}

impl<W: Write> TupleSink for CsvOutputStream<W> {
    fn write_tuple(&mut self, tuple: &[Field]) -> std::io::Result<()> {
        if !self.header_written {
            if let Some(ref h) = self.header {
                writeln!(self.writer, "{}", h)?;
            }
            self.header_written = true;
        }
        let items: Vec<String> = tuple
            .iter()
            .map(|f| match f {
                Field::UInt(u) => u.to_string(),
                Field::Int(i) => i.to_string(),
                Field::Float(fl) => format!("{}", fl),
                Field::Bytes(b) => {
                    let s = String::from_utf8_lossy(b);
                    if s.contains(',') || s.contains('"') || s.contains('\n') {
                        format!("\"{}\"", s.replace('"', "\"\""))
                    } else {
                        s.to_string()
                    }
                }
            })
            .collect();
        writeln!(self.writer, "{}", items.join(","))?;
        Ok(())
    }

    fn flush_stream(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

const ANSI_COLORS: &[&str] = &[
    "\x1b[36m", // Cyan
    "\x1b[33m", // Yellow
    "\x1b[32m", // Green
    "\x1b[35m", // Magenta
    "\x1b[34m", // Blue
    "\x1b[96m", // Bright Cyan
];
const ANSI_RESET: &str = "\x1b[0m";

/// Colorized terminal dumper that visually color-codes and annotates tuple fields.
pub struct VisualOutputStream<W> {
    writer: W,
    unit_index: usize,
}

impl<W: Write> VisualOutputStream<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            unit_index: 0,
        }
    }
}

impl<W: Write> TupleSink for VisualOutputStream<W> {
    fn write_tuple(&mut self, tuple: &[Field]) -> std::io::Result<()> {
        write!(self.writer, "[#{:04}] ", self.unit_index)?;
        self.unit_index += 1;

        for (i, field) in tuple.iter().enumerate() {
            let color = ANSI_COLORS[i % ANSI_COLORS.len()];
            write!(self.writer, "{}[f{}: {}]{} ", color, i, field, ANSI_RESET)?;
        }
        writeln!(self.writer)?;
        Ok(())
    }

    fn flush_stream(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_output_sink() {
        let mut buf = Vec::new();
        {
            let mut sink = FileOutputStream::new(&mut buf, false, false);
            sink.write_bits(BigUint::from(0x12u32), 8).unwrap();
            sink.write_bits(BigUint::from(0x34u32), 8).unwrap();
            sink.flush_stream().unwrap();
        }
        assert_eq!(buf, vec![0x12, 0x34]);
    }

    #[test]
    fn test_hex_output_sink() {
        let mut buf = Vec::new();
        {
            let mut sink = HexOutputStream::new(&mut buf);
            sink.write_bits(BigUint::from(5u32), 8).unwrap();
            sink.flush_stream().unwrap();
        }
        assert_eq!(String::from_utf8(buf).unwrap(), "05\n");

        let mut multi_buf = Vec::new();
        {
            let mut sink = HexOutputStream::new(&mut multi_buf);
            sink.write_bits(BigUint::from(0x0Au32), 8).unwrap();
            sink.write_bits(BigUint::from(0xBCu32), 8).unwrap();
            sink.flush_stream().unwrap();
        }
        assert_eq!(String::from_utf8(multi_buf).unwrap(), "0a bc\n");
    }

    #[test]
    fn test_bit_output_sink() {
        let mut buf = Vec::new();
        {
            let mut sink = BitOutputStream::new(&mut buf);
            sink.write_bits(BigUint::from(1u32), 3).unwrap();
            sink.write_bits(BigUint::from(6u32), 3).unwrap();
            sink.flush_stream().unwrap();
        }
        assert_eq!(String::from_utf8(buf).unwrap(), "001 110\n");
    }

    #[test]
    fn test_large_unit_sinks() {
        // Hex sink with 256-bit value
        let val_256: BigUint = (BigUint::one() << 255usize) | BigUint::from(0xABu32);
        let mut hex_buf = Vec::new();
        {
            let mut sink = HexOutputStream::new(&mut hex_buf);
            sink.write_bits(val_256.clone(), 256).unwrap();
            sink.flush_stream().unwrap();
        }
        let hex_str = String::from_utf8(hex_buf).unwrap();
        // 256 bits = 64 hex characters + trailing space + newline
        assert_eq!(hex_str.trim().len(), 64);
        assert!(hex_str.starts_with("8000"));
        assert!(hex_str.trim().ends_with("00ab"));

        // Bit sink with 128 bits
        let val_128: BigUint = (BigUint::one() << 127usize) | BigUint::one();
        let mut bit_buf = Vec::new();
        {
            let mut sink = BitOutputStream::new(&mut bit_buf);
            sink.write_bits(val_128, 128).unwrap();
            sink.flush_stream().unwrap();
        }
        let bit_str = String::from_utf8(bit_buf).unwrap();
        assert_eq!(bit_str.trim().len(), 128);
        assert!(bit_str.starts_with('1'));
        assert!(bit_str.trim().ends_with('1'));

        // Integer sink with 256-bit number
        let mut int_buf = Vec::new();
        {
            let mut sink = IntegerOutputStream::new(&mut int_buf);
            sink.write_bits(val_256.clone(), 256).unwrap();
            sink.flush_stream().unwrap();
        }
        let int_str = String::from_utf8(int_buf).unwrap();
        assert_eq!(int_str.trim(), val_256.to_string());

        // Tuple sink with 500 fields
        let tuple_500: Vec<Field> = (0..500)
            .map(|i| Field::UInt(BigUint::from(i as u32)))
            .collect();
        let mut tuple_buf = Vec::new();
        {
            let mut sink = TupleDirectOutput::new(&mut tuple_buf);
            sink.write_tuple(&tuple_500).unwrap();
            sink.flush_stream().unwrap();
        }
        let tuple_str = String::from_utf8(tuple_buf).unwrap();
        let parts: Vec<&str> = tuple_str.trim().split(',').collect();
        assert_eq!(parts.len(), 500);
        assert_eq!(parts[0], "0");
        assert_eq!(parts[499], "499");
    }

    #[test]
    fn test_structured_sinks() {
        let tuple = vec![
            Field::UInt(BigUint::from(10u32)),
            Field::Float(3.5),
            Field::Bytes(b"test".to_vec()),
        ];

        // JSON sink (array)
        let mut json_buf = Vec::new();
        {
            let mut sink = JsonOutputStream::new(&mut json_buf, None);
            sink.write_tuple(&tuple).unwrap();
            sink.flush_stream().unwrap();
        }
        let json_str = String::from_utf8(json_buf).unwrap();
        assert_eq!(json_str.trim(), "[10,3.5,\"test\"]");

        // JSON sink (object with field names)
        let mut json_obj_buf = Vec::new();
        {
            let mut sink = JsonOutputStream::new(
                &mut json_obj_buf,
                Some(vec![
                    "count".to_string(),
                    "val".to_string(),
                    "tag".to_string(),
                ]),
            );
            sink.write_tuple(&tuple).unwrap();
            sink.flush_stream().unwrap();
        }
        let json_obj_str = String::from_utf8(json_obj_buf).unwrap();
        assert_eq!(
            json_obj_str.trim(),
            "{\"count\":10,\"tag\":\"test\",\"val\":3.5}"
        );

        // CSV sink
        let mut csv_buf = Vec::new();
        {
            let mut sink = CsvOutputStream::new(&mut csv_buf, Some("a,b,c".to_string()));
            sink.write_tuple(&tuple).unwrap();
            sink.flush_stream().unwrap();
        }
        let csv_str = String::from_utf8(csv_buf).unwrap();
        assert_eq!(csv_str.trim(), "a,b,c\n10,3.5,test");
    }
}
