use crate::counter::Counter;
use crate::error::BddError;
use crate::field::{reverse_bits, Field};
use num_bigint::{BigInt, BigUint};
use num_traits::{One, Signed, Zero};
use rand::RngCore;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

/// Stream configuration parameters for bit extraction.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub skip_bits: u64,
    pub skip_units: u64,
    pub gap: u64,
    pub assert_aligned: bool,
    pub drop_partial_eof: bool,
    pub reverse_bytes: bool,
    pub reverse_unit: bool,
    pub unit_size: usize,
    pub seek_allowed: bool,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            skip_bits: 0,
            skip_units: 0,
            gap: 0,
            assert_aligned: false,
            drop_partial_eof: false,
            reverse_bytes: false,
            reverse_unit: false,
            unit_size: 8,
            seek_allowed: true,
        }
    }
}

/// Helper trait combining Read and Seek.
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

/// Trait for streams that may attempt seeking forward before falling back to reading.
pub trait StreamSeek {
    /// Attempts to seek forward by `bytes` relative to current position.
    /// Returns Ok(true) if seek succeeded.
    /// Returns Ok(false) if seeking is unsupported, failed with ESPIPE, or disabled.
    /// Returns Err(e) on fatal I/O error.
    fn try_seek(&mut self, _bytes: u64) -> std::io::Result<bool> {
        Ok(false)
    }
}

/// Internal source enum for BddReader.
pub enum ReaderSource {
    Seekable(Box<dyn ReadSeek>),
    Streaming(Box<dyn Read>),
}

/// Unified reader for bdd that automatically seeks when backed by a seekable file/descriptor,
/// with graceful fallback to streaming sequential reads for pipes/fifos.
pub struct BddReader {
    source: ReaderSource,
    seek_allowed: bool,
}

impl BddReader {
    pub fn new_seekable<R: ReadSeek + 'static>(reader: R, seek_allowed: bool) -> Self {
        Self {
            source: ReaderSource::Seekable(Box::new(reader)),
            seek_allowed,
        }
    }

    pub fn new_streaming<R: Read + 'static>(reader: R) -> Self {
        Self {
            source: ReaderSource::Streaming(Box::new(reader)),
            seek_allowed: false,
        }
    }

    pub fn from_file(file: std::fs::File, seek_allowed: bool) -> Self {
        Self::new_seekable(BufReader::new(file), seek_allowed)
    }

    pub fn from_stdin(seek_allowed: bool) -> Self {
        #[cfg(unix)]
        {
            use std::os::fd::AsFd;
            if let Ok(owned) = std::io::stdin().as_fd().try_clone_to_owned() {
                let mut f = std::fs::File::from(owned);
                if f.stream_position().is_ok() {
                    return Self::new_seekable(BufReader::new(f), seek_allowed);
                }
            }
        }
        Self::new_streaming(std::io::stdin())
    }

    pub fn is_seekable(&self) -> bool {
        matches!(self.source, ReaderSource::Seekable(_))
    }
}

impl Read for BddReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.source {
            ReaderSource::Seekable(s) => s.read(buf),
            ReaderSource::Streaming(s) => s.read(buf),
        }
    }
}

impl StreamSeek for BddReader {
    fn try_seek(&mut self, bytes: u64) -> std::io::Result<bool> {
        if !self.seek_allowed || bytes == 0 {
            return Ok(false);
        }
        match &mut self.source {
            ReaderSource::Seekable(s) => match s.seek(SeekFrom::Current(bytes as i64)) {
                Ok(_) => Ok(true),
                Err(e) if e.raw_os_error() == Some(29) => Ok(false),
                Err(_) => Ok(false),
            },
            ReaderSource::Streaming(_) => Ok(false),
        }
    }
}

impl<T: AsRef<[u8]>> StreamSeek for std::io::Cursor<T> {
    fn try_seek(&mut self, bytes: u64) -> std::io::Result<bool> {
        match self.seek(SeekFrom::Current(bytes as i64)) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

impl StreamSeek for std::fs::File {
    fn try_seek(&mut self, bytes: u64) -> std::io::Result<bool> {
        match self.seek(SeekFrom::Current(bytes as i64)) {
            Ok(_) => Ok(true),
            Err(e) if e.raw_os_error() == Some(29) => Ok(false),
            Err(_) => Ok(false),
        }
    }
}

impl<T: Read + Seek> StreamSeek for BufReader<T> {
    fn try_seek(&mut self, bytes: u64) -> std::io::Result<bool> {
        match self.seek(SeekFrom::Current(bytes as i64)) {
            Ok(_) => Ok(true),
            Err(e) if e.raw_os_error() == Some(29) => Ok(false),
            Err(_) => Ok(false),
        }
    }
}

impl StreamSeek for &[u8] {
    fn try_seek(&mut self, _bytes: u64) -> std::io::Result<bool> {
        Ok(false)
    }
}

impl StreamSeek for std::io::Stdin {
    fn try_seek(&mut self, _bytes: u64) -> std::io::Result<bool> {
        Ok(false)
    }
}

impl StreamSeek for Box<dyn Read> {
    fn try_seek(&mut self, _bytes: u64) -> std::io::Result<bool> {
        Ok(false)
    }
}

impl StreamSeek for Box<dyn ReadSeek> {
    fn try_seek(&mut self, bytes: u64) -> std::io::Result<bool> {
        match self.seek(SeekFrom::Current(bytes as i64)) {
            Ok(_) => Ok(true),
            Err(e) if e.raw_os_error() == Some(29) => Ok(false),
            Err(_) => Ok(false),
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

impl<R: Read + StreamSeek> FileInputStream<R> {
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
        let total_skip_bits: u64 = self
            .config
            .skip_bits
            .saturating_add((self.config.unit_size as u64).saturating_mul(self.config.skip_units));
        let skip_bytes = total_skip_bits / 8;
        if skip_bytes > 0 {
            let mut remaining = skip_bytes;
            if self.config.seek_allowed {
                if let Ok(true) = self.reader.try_seek(skip_bytes) {
                    remaining = 0;
                }
            }
            if remaining > 0 {
                let mut discard = [0u8; 65536];
                while remaining > 0 {
                    let to_read = (remaining.min(discard.len() as u64)) as usize;
                    match self.reader.read(&mut discard[..to_read]) {
                        Ok(0) | Err(_) => {
                            self.eof = true;
                            break;
                        }
                        Ok(n) => {
                            remaining -= n as u64;
                        }
                    }
                }
            }
        }
        let rem = (total_skip_bits % 8) as usize;
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

    pub fn skip_gap(&mut self, gap: u64) {
        if gap == 0 {
            return;
        }

        if (self.bits_in_buffer as u64) >= gap {
            self.bits_in_buffer -= gap as usize;
            let mask = if self.bits_in_buffer > 0 {
                (BigUint::one() << self.bits_in_buffer) - 1u32
            } else {
                BigUint::zero()
            };
            self.buffer &= mask;
            return;
        }

        let remaining_gap = gap - (self.bits_in_buffer as u64);
        self.bits_in_buffer = 0;
        self.buffer = BigUint::zero();

        let gap_bytes = remaining_gap / 8;
        let rem_bits = (remaining_gap % 8) as usize;

        if gap_bytes > 0 {
            let mut remaining = gap_bytes;
            if self.config.seek_allowed {
                if let Ok(true) = self.reader.try_seek(gap_bytes) {
                    remaining = 0;
                }
            }
            if remaining > 0 {
                let mut discard = [0u8; 65536];
                while remaining > 0 {
                    let to_read = (remaining.min(discard.len() as u64)) as usize;
                    match self.reader.read(&mut discard[..to_read]) {
                        Ok(0) | Err(_) => {
                            self.eof = true;
                            return;
                        }
                        Ok(n) => {
                            remaining -= n as u64;
                        }
                    }
                }
            }
        }

        if rem_bits > 0 {
            let b = self.read_byte();
            if self.eof {
                return;
            }
            self.bits_in_buffer = 8 - rem_bits;
            let mask = (BigUint::one() << self.bits_in_buffer) - 1u32;
            self.buffer = BigUint::from(b) & mask;
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

impl<R: Read + StreamSeek> UnitStream for FileInputStream<R> {
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
                if self.eof && self.config.drop_partial_eof {
                    self.bits_in_buffer = 0;
                    self.buffer = BigUint::zero();
                    return Ok(None);
                }
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

            if self.bits_in_buffer == 0 && self.config.gap == 0 {
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

            if self.config.gap > 0 {
                self.skip_gap(self.config.gap);
            }

            if self.bits_in_buffer == 0 && !self.eof {
                let b = self.read_byte();
                if !self.eof {
                    self.buffer = BigUint::from(b);
                    self.bits_in_buffer = 8;
                }
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
    remaining: Option<u64>,
}

impl ZeroStream {
    pub fn new(counter: Counter) -> Self {
        Self {
            remaining: counter.count,
        }
    }
}

impl UnitStream for ZeroStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        if let Some(rem) = &mut self.remaining {
            if *rem == 0 {
                return Ok(None);
            }
            *rem -= 1;
        }
        Ok(Some(BigUint::zero()))
    }
}

pub struct OneStream {
    remaining: Option<u64>,
    pub unit_size: usize,
}

impl OneStream {
    pub fn new(counter: Counter, unit_size: usize) -> Self {
        Self {
            remaining: counter.count,
            unit_size,
        }
    }
}

impl UnitStream for OneStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        if let Some(rem) = &mut self.remaining {
            if *rem == 0 {
                return Ok(None);
            }
            *rem -= 1;
        }
        Ok(Some((BigUint::one() << self.unit_size) - 1u32))
    }
}

pub struct RandomStream {
    remaining: Option<u64>,
    pub unit_size: usize,
}

impl RandomStream {
    pub fn new(counter: Counter, unit_size: usize) -> Self {
        let mut s = Self {
            remaining: counter.count,
            unit_size,
        };
        for _ in 0..counter.skip {
            let _ = s.next_unit();
        }
        s
    }
}

impl UnitStream for RandomStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        if let Some(rem) = &mut self.remaining {
            if *rem == 0 {
                return Ok(None);
            }
            *rem -= 1;
        }
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
    remaining: Option<u64>,
    pub unit_size: usize,
    current_val: u64,
}

impl CounterStream {
    pub fn new(counter: Counter, unit_size: usize) -> Self {
        Self {
            remaining: counter.count,
            unit_size,
            current_val: counter.skip,
        }
    }
}

impl UnitStream for CounterStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        if let Some(rem) = &mut self.remaining {
            if *rem == 0 {
                return Ok(None);
            }
            *rem -= 1;
        }
        let mask = (BigUint::one() << self.unit_size) - 1u32;
        let val = BigUint::from(self.current_val) & mask;
        self.current_val = self.current_val.wrapping_add(1);
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

/// Parses a comma-separated tuple line, distinguishing quoted strings from raw unquoted numbers.
pub fn parse_csv_tuple_line(line: &str) -> Vec<(String, bool)> {
    let mut fields = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        // Skip leading whitespace (except newlines)
        while i < n && chars[i].is_whitespace() && chars[i] != '\n' && chars[i] != '\r' {
            i += 1;
        }
        if i >= n {
            break;
        }

        if chars[i] == '"' || chars[i] == '\'' {
            let quote = chars[i];
            i += 1;
            let mut buf = String::new();
            while i < n {
                if chars[i] == quote {
                    if i + 1 < n && chars[i + 1] == quote {
                        buf.push(quote);
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else if chars[i] == '\\' && i + 1 < n && chars[i + 1] == quote {
                    buf.push(quote);
                    i += 2;
                } else {
                    buf.push(chars[i]);
                    i += 1;
                }
            }
            while i < n && chars[i] != ',' {
                i += 1;
            }
            if i < n && chars[i] == ',' {
                i += 1;
            }
            fields.push((buf, true));
        } else {
            let mut buf = String::new();
            while i < n && chars[i] != ',' {
                buf.push(chars[i]);
                i += 1;
            }
            if i < n && chars[i] == ',' {
                i += 1;
            }
            fields.push((buf.trim().to_string(), false));
        }
    }

    if !line.is_empty() && line.trim_end().ends_with(',') {
        fields.push((String::new(), false));
    }

    fields
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
                    let parsed_fields = parse_csv_tuple_line(trimmed);
                    let mut tuple = Vec::new();
                    for (field_str, is_quoted) in parsed_fields {
                        if is_quoted {
                            tuple.push(Field::Bytes(field_str.into_bytes()));
                        } else if let Ok(bi) = field_str.parse::<BigInt>() {
                            if bi.is_negative() {
                                tuple.push(Field::Int(bi));
                            } else {
                                tuple.push(Field::UInt(bi.to_biguint().unwrap()));
                            }
                        } else if let Ok(fl) = field_str.parse::<f64>() {
                            tuple.push(Field::Float(fl));
                        } else {
                            tuple.push(Field::Bytes(field_str.into_bytes()));
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

    #[test]
    fn test_file_stream_seek_auto() {
        let data = vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60];
        let cursor = std::io::Cursor::new(data);
        let config = StreamConfig {
            skip_bits: 0,
            skip_units: 3, // skip 3 units of 8 bits = 24 bits = 3 bytes
            unit_size: 8,
            seek_allowed: true,
            ..Default::default()
        };
        let mut stream = FileInputStream::new(cursor, config, Counter::new(0, None));
        stream.do_skip();
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(0x40u32)));
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(0x50u32)));
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(0x60u32)));
        assert_eq!(stream.next_unit().unwrap(), None);
    }

    #[test]
    fn test_file_stream_no_seek_fallback() {
        let data = vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60];
        let cursor = std::io::Cursor::new(data);
        let config = StreamConfig {
            skip_bits: 0,
            skip_units: 3,
            unit_size: 8,
            seek_allowed: false, // seeking explicitly disabled
            ..Default::default()
        };
        let mut stream = FileInputStream::new(cursor, config, Counter::new(0, None));
        stream.do_skip();
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(0x40u32)));
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(0x50u32)));
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(0x60u32)));
        assert_eq!(stream.next_unit().unwrap(), None);
    }

    #[test]
    fn test_stream_sub_byte_skip_with_seek() {
        // Data: 0xFF, 0x80 (11111111 10000000)
        // Skip 10 bits = 1 byte (8 bits) + 2 bits
        // Remaining in second byte: 6 bits (000000)
        let data = vec![0xFF, 0x80];
        let cursor = std::io::Cursor::new(data);
        let config = StreamConfig {
            skip_bits: 10,
            skip_units: 0,
            unit_size: 6,
            seek_allowed: true,
            ..Default::default()
        };
        let mut stream = FileInputStream::new(cursor, config, Counter::new(0, None));
        stream.do_skip();
        assert_eq!(stream.next_unit().unwrap(), Some(BigUint::from(0u32)));
    }

    #[test]
    fn test_bdd_reader_seeking_and_streaming() {
        let data = vec![1, 2, 3, 4, 5];
        let mut seekable = BddReader::new_seekable(std::io::Cursor::new(data.clone()), true);
        assert!(seekable.is_seekable());
        assert!(seekable.try_seek(2).unwrap());
        let mut buf = [0u8; 2];
        seekable.read_exact(&mut buf).unwrap();
        assert_eq!(buf, [3, 4]);

        let mut streaming = BddReader::new_streaming(std::io::Cursor::new(data));
        assert!(!streaming.is_seekable());
        assert!(!streaming.try_seek(2).unwrap());
    }

    #[test]
    fn test_drop_partial_eof() {
        // 1 byte: 0b110010_11 (0xCB)
        // Unit size: 6 bits.
        // First 6 bits: 110010 = 50
        // Trailing 2 bits: 11
        let data = vec![0xCBu8];

        // Without drop_partial_eof: trailing 2 bits padded with 4 zeros: 110000 = 48
        let config_pad = StreamConfig {
            unit_size: 6,
            drop_partial_eof: false,
            ..Default::default()
        };
        let mut stream_pad = FileInputStream::new(
            std::io::Cursor::new(data.clone()),
            config_pad,
            Counter::new(0, None),
        );
        stream_pad.do_skip();
        assert_eq!(stream_pad.next_unit().unwrap(), Some(BigUint::from(50u32)));
        assert_eq!(stream_pad.next_unit().unwrap(), Some(BigUint::from(48u32)));
        assert_eq!(stream_pad.next_unit().unwrap(), None);

        // With drop_partial_eof: trailing 2 bits are dropped!
        let config_drop = StreamConfig {
            unit_size: 6,
            drop_partial_eof: true,
            ..Default::default()
        };
        let mut stream_drop = FileInputStream::new(
            std::io::Cursor::new(data),
            config_drop,
            Counter::new(0, None),
        );
        stream_drop.do_skip();
        assert_eq!(stream_drop.next_unit().unwrap(), Some(BigUint::from(50u32)));
        assert_eq!(stream_drop.next_unit().unwrap(), None);
    }
}
