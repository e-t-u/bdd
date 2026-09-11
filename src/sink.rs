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

        let s = format!("{}{h} ", filler);
        self.writer.write_all(s.as_bytes())?;
        self.output_column += hex_digits + 1;
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

        let s = format!("{}{b_str} ", filler);
        self.writer.write_all(s.as_bytes())?;
        self.output_column += bits + 1;
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
pub struct TupleDirectOutput<W> {
    writer: W,
}

impl<W: Write> TupleDirectOutput<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn write_tuple(&mut self, tuple: &[Field]) -> std::io::Result<()> {
        let str_list: Vec<String> = tuple.iter().map(|f| f.to_string()).collect();
        writeln!(self.writer, "{}", str_list.join(","))?;
        Ok(())
    }

    pub fn flush_stream(&mut self) -> std::io::Result<()> {
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
        assert_eq!(String::from_utf8(buf).unwrap(), "05 \n");
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
}
