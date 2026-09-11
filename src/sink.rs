use crate::field::{reverse_bits, Field};
use num_bigint::BigUint;
use num_traits::{One, ToPrimitive, Zero};
use std::io::Write;

pub trait UnitSink {
    fn write_bits(&mut self, unit: BigUint, bits: usize) -> std::io::Result<()>;
    fn flush_stream(&mut self) -> std::io::Result<()>;
}

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
            let mut val = (std::mem::take(&mut self.buffer) << shift).to_u8().unwrap_or(0);
            if self.reverse_bytes {
                val = val.reverse_bits();
            }
            self.writer.write_all(&[val])?;
            self.bits_in_buffer = 0;
        }
        self.writer.flush()
    }
}

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
