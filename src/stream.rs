use crate::counter::Counter;
use crate::error::BddError;
use crate::field::{reverse_bits, Field};
use num_bigint::{BigInt, BigUint};
use num_traits::{One, Signed, Zero};
use rand::RngCore;
use std::io::{BufRead, Read};

/// Stream configuration parameters for bit extraction.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub skip_bits: usize,
    pub skip_units: usize,
    pub gap: usize,
    pub assert_aligned: bool,
    pub reverse_bytes: bool,
    pub reverse_unit: bool,
    pub unit_size: usize,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            skip_bits: 0,
            skip_units: 0,
            gap: 0,
            assert_aligned: false,
            reverse_bytes: false,
            reverse_unit: false,
            unit_size: 8,
        }
    }
}

/// Abstract iterator yielding units from an input source.
pub trait UnitStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError>;
}

/// Bitstream reader extracting variable-width units from an underlying reader.
pub struct FileInputStream<R> {
    pub reader: R,
    pub buffer: BigUint,
    pub bits_in_buffer: usize,
    pub eof: bool,
    pub config: StreamConfig,
    pub counter: Counter,
}

impl<R: Read> FileInputStream<R> {
    pub fn new(reader: R, config: StreamConfig, counter: Counter) -> Self {
        Self {
            reader,
            buffer: BigUint::zero(),
            bits_in_buffer: 0,
            eof: false,
            config,
            counter,
        }
    }

    fn read_byte(&mut self) -> u8 {
        let mut buf = [0u8; 1];
        match self.reader.read(&mut buf) {
            Ok(1) => {
                let mut val = buf[0];
                if self.config.reverse_bytes {
                    val = val.reverse_bits();
                }
                val
            }
            _ => {
                self.eof = true;
                0
            }
        }
    }

    pub fn do_skip(&mut self) {
        let total_skip_bits =
            self.config.skip_bits + (self.config.unit_size * self.config.skip_units);
        let skip_bytes = total_skip_bits / 8;
        if skip_bytes > 0 {
            let mut remaining = skip_bytes;
            let mut discard = [0u8; 4096];
            while remaining > 0 {
                let to_read = remaining.min(discard.len());
                match self.reader.read(&mut discard[..to_read]) {
                    Ok(0) | Err(_) => {
                        self.eof = true;
                        break;
                    }
                    Ok(n) => {
                        remaining -= n;
                    }
                }
            }
        }
        let rem = total_skip_bits % 8;
        if rem != 0 {
            self.bits_in_buffer = 8 - rem;
            let b = self.read_byte();
            let mask = (BigUint::one() << self.bits_in_buffer) - 1u32;
            self.buffer = BigUint::from(b) & mask;
        } else {
            let b = self.read_byte();
            self.buffer = BigUint::from(b);
            self.bits_in_buffer = 8;
        }
    }

    pub fn read_bits(&mut self, bits: usize) -> BigUint {
        while self.bits_in_buffer < bits {
            let b = self.read_byte();
            self.buffer = (std::mem::take(&mut self.buffer) << 8) | BigUint::from(b);
            self.bits_in_buffer += 8;
        }
        let right_edge = self.bits_in_buffer - bits;
        let mut unit = &self.buffer >> right_edge;
        if self.config.reverse_unit {
            unit = reverse_bits(&unit, self.config.unit_size);
        }
        self.bits_in_buffer -= bits;
        let mask = if self.bits_in_buffer > 0 {
            (BigUint::one() << self.bits_in_buffer) - 1u32
        } else {
            BigUint::zero()
        };
        self.buffer &= mask;
        unit
    }
}

impl<R: Read> UnitStream for FileInputStream<R> {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        if self.eof {
            return Ok(None);
        }

        loop {
            if self.eof {
                if self.config.assert_aligned {
                    return Err(BddError::NonAlignedEof);
                }
                return Ok(None);
            }

            while self.bits_in_buffer < self.config.unit_size {
                let b = self.read_byte();
                self.buffer = (std::mem::take(&mut self.buffer) << 8) | BigUint::from(b);
                self.bits_in_buffer += 8;
            }

            let right_edge = self.bits_in_buffer - self.config.unit_size;
            let mut unit = &self.buffer >> right_edge;

            self.counter.next();
            let included = self.counter.included();
            if self.counter.finished() {
                self.eof = true;
            }

            self.bits_in_buffer -= self.config.unit_size;
            let mask = if self.bits_in_buffer > 0 {
                (BigUint::one() << self.bits_in_buffer) - 1u32
            } else {
                BigUint::zero()
            };
            self.buffer &= mask;

            if self.bits_in_buffer == 0 {
                let b = self.read_byte();
                if self.eof {
                    if included {
                        if self.config.reverse_unit {
                            unit = reverse_bits(&unit, self.config.unit_size);
                        }
                        return Ok(Some(unit));
                    } else {
                        return Ok(None);
                    }
                }
                self.buffer = BigUint::from(b);
                self.bits_in_buffer = 8;
            }

            while self.bits_in_buffer < self.config.gap {
                let b = self.read_byte();
                self.buffer = (std::mem::take(&mut self.buffer) << 8) | BigUint::from(b);
                self.bits_in_buffer += 8;
            }
            self.bits_in_buffer -= self.config.gap;
            let mask = if self.bits_in_buffer > 0 {
                (BigUint::one() << self.bits_in_buffer) - 1u32
            } else {
                BigUint::zero()
            };
            self.buffer &= mask;

            if self.bits_in_buffer == 0 {
                let b = self.read_byte();
                if self.eof {
                    if included {
                        if self.config.reverse_unit {
                            unit = reverse_bits(&unit, self.config.unit_size);
                        }
                        return Ok(Some(unit));
                    } else {
                        return Ok(None);
                    }
                }
                self.buffer = BigUint::from(b);
                self.bits_in_buffer = 8;
            }

            if included {
                if self.config.reverse_unit {
                    unit = reverse_bits(&unit, self.config.unit_size);
                }
                return Ok(Some(unit));
            }
        }
    }
}

pub struct ZeroStream {
    remaining: usize,
}

impl ZeroStream {
    pub fn new(counter: Counter) -> Self {
        Self {
            remaining: counter.count.unwrap_or(0),
        }
    }
}

impl UnitStream for ZeroStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        Ok(Some(BigUint::zero()))
    }
}

pub struct OneStream {
    remaining: usize,
    pub unit_size: usize,
}

impl OneStream {
    pub fn new(counter: Counter, unit_size: usize) -> Self {
        Self {
            remaining: counter.count.unwrap_or(0),
            unit_size,
        }
    }
}

impl UnitStream for OneStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        Ok(Some((BigUint::one() << self.unit_size) - 1u32))
    }
}

pub struct RandomStream {
    remaining: usize,
    pub unit_size: usize,
}

impl RandomStream {
    pub fn new(counter: Counter, unit_size: usize) -> Self {
        Self {
            remaining: counter.count.unwrap_or(0),
            unit_size,
        }
    }
}

impl UnitStream for RandomStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        let mut rng = rand::thread_rng();
        let num_bytes = self.unit_size / 8 + 1;
        let mut buf = vec![0u8; num_bytes];
        rng.fill_bytes(&mut buf);
        let mut val = BigUint::from_bytes_be(&buf);
        let mask = (BigUint::one() << self.unit_size) - 1u32;
        val &= mask;
        Ok(Some(val))
    }
}

pub struct CounterStream {
    remaining: usize,
    pub unit_size: usize,
    current_val: usize,
}

impl CounterStream {
    pub fn new(counter: Counter, unit_size: usize) -> Self {
        Self {
            remaining: counter.count.unwrap_or(0),
            unit_size,
            current_val: counter.skip,
        }
    }
}

impl UnitStream for CounterStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        let mask = (BigUint::one() << self.unit_size) - 1u32;
        let val = BigUint::from(self.current_val) & mask;
        self.current_val += 1;
        Ok(Some(val))
    }
}

pub struct IntegerInputStream<R> {
    reader: R,
    counter: Counter,
}

impl<R: BufRead> IntegerInputStream<R> {
    pub fn new(reader: R, counter: Counter) -> Self {
        Self { reader, counter }
    }
}

impl<R: BufRead> UnitStream for IntegerInputStream<R> {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        let mut line = String::new();
        loop {
            line.clear();
            match self.reader.read_line(&mut line) {
                Ok(0) => return Ok(None),
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    self.counter.next();
                    if self.counter.finished() {
                        return Ok(None);
                    }
                    if !self.counter.included() {
                        continue;
                    }
                    let bi = match trimmed.parse::<BigInt>() {
                        Ok(i) => i,
                        Err(_) => {
                            eprintln!("Non-integer '{}' interpreted as zero", trimmed);
                            BigInt::zero()
                        }
                    };
                    let val = if bi.is_negative() {
                        eprintln!("Negative integer '{}' interpreted as positive", trimmed);
                        bi.abs().to_biguint().unwrap_or_default()
                    } else {
                        bi.to_biguint().unwrap_or_default()
                    };
                    return Ok(Some(val));
                }
                Err(e) => return Err(BddError::from(e)),
            }
        }
    }
}

pub struct TupleDirectInput<R> {
    reader: R,
    counter: Counter,
}

impl<R: BufRead> TupleDirectInput<R> {
    pub fn new(reader: R, counter: Counter) -> Self {
        Self { reader, counter }
    }

    pub fn next_tuple(&mut self) -> Result<Option<Vec<Field>>, BddError> {
        let mut raw_bytes = Vec::new();
        loop {
            raw_bytes.clear();
            match self.reader.read_until(b'\n', &mut raw_bytes) {
                Ok(0) => return Ok(None),
                Ok(_) => {
                    let lossy = String::from_utf8_lossy(&raw_bytes);
                    let trimmed = lossy.trim_end_matches(&['\r', '\n'][..]);
                    if trimmed.is_empty() {
                        continue;
                    }
                    self.counter.next();
                    if self.counter.finished() {
                        return Ok(None);
                    }
                    if !self.counter.included() {
                        continue;
                    }
                    let mut rdr = csv::ReaderBuilder::new()
                        .has_headers(false)
                        .flexible(true)
                        .trim(csv::Trim::All)
                        .from_reader(trimmed.as_bytes());

                    let mut tuple = Vec::new();
                    if let Some(Ok(record)) = rdr.records().next() {
                        for field_str in record.iter() {
                            let f_trim = field_str.trim();
                            if let Ok(bi) = f_trim.parse::<BigInt>() {
                                if bi.is_negative() {
                                    tuple.push(Field::Int(bi));
                                } else {
                                    tuple.push(Field::UInt(bi.to_biguint().unwrap()));
                                }
                            } else if let Ok(fl) = f_trim.parse::<f64>() {
                                tuple.push(Field::Float(fl));
                            } else {
                                tuple.push(Field::Bytes(field_str.as_bytes().to_vec()));
                            }
                        }
                    }
                    return Ok(Some(tuple));
                }
                Err(e) => return Err(BddError::from(e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_stream() {
        let counter = Counter::new(2, Some(3));
        let mut stream = CounterStream::new(counter, 8);
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(2u32)));
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(3u32)));
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(4u32)));
        assert_eq!(stream.next_unit().unwrap(), None);
    }

    #[test]
    fn test_file_stream_bytes() {
        let data = [0x12, 0x34];
        let config = StreamConfig {
            unit_size: 8,
            ..Default::default()
        };
        let mut stream = FileInputStream::new(&data[..], config, Counter::new(0, None));
        stream.do_skip();
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(0x12u32)));
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(0x34u32)));
        assert_eq!(stream.next_unit().unwrap(), None);
    }
}
