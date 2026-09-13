use crate::bits::BitValue;
use crate::counter::Counter;
use crate::error::BddError;
use crate::field::{parse_radix_bigint, reverse_bits, reverse_bits_u64, Field};
use num_bigint::{BigInt, BigUint};
use num_traits::{One, Signed, ToPrimitive, Zero};
use rand::RngCore;
use std::fs::File;
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
    pub repeat_count: usize,
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
            repeat_count: 1,
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

    /// Attempts to rewind the stream to the beginning (offset 0).
    /// Returns Ok(true) if rewind succeeded.
    /// Returns Ok(false) if rewinding is unsupported.
    fn rewind(&mut self) -> std::io::Result<bool> {
        Ok(false)
    }
}

/// Internal source enum for BddReader.
pub enum ReaderSource {
    Seekable(Box<dyn ReadSeek>),
    Streaming(Box<dyn Read>),
    #[cfg(feature = "mmap")]
    Mmap(std::io::Cursor<memmap2::Mmap>),
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

    #[cfg(feature = "mmap")]
    pub fn new_mmap(mmap: memmap2::Mmap, seek_allowed: bool) -> Self {
        Self {
            source: ReaderSource::Mmap(std::io::Cursor::new(mmap)),
            seek_allowed,
        }
    }

    pub fn from_file(file: std::fs::File, seek_allowed: bool) -> Self {
        Self::from_file_with_mmap(file, seek_allowed, true)
    }

    pub fn from_file_with_mmap(file: std::fs::File, seek_allowed: bool, allow_mmap: bool) -> Self {
        #[cfg(feature = "mmap")]
        if allow_mmap {
            if let Ok(meta) = file.metadata() {
                if meta.is_file() && meta.len() > 0 {
                    if let Ok(mmap) = unsafe { memmap2::MmapOptions::new().map(&file) } {
                        #[cfg(unix)]
                        let _ = mmap.advise(memmap2::Advice::Sequential);
                        return Self {
                            source: ReaderSource::Mmap(std::io::Cursor::new(mmap)),
                            seek_allowed,
                        };
                    }
                }
            }
        }
        let _ = allow_mmap;
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
        match self.source {
            ReaderSource::Seekable(_) => true,
            #[cfg(feature = "mmap")]
            ReaderSource::Mmap(_) => true,
            ReaderSource::Streaming(_) => false,
        }
    }

    pub fn is_mmap(&self) -> bool {
        #[cfg(feature = "mmap")]
        {
            matches!(self.source, ReaderSource::Mmap(_))
        }
        #[cfg(not(feature = "mmap"))]
        {
            false
        }
    }

    pub fn as_slice(&self) -> Option<&[u8]> {
        #[cfg(feature = "mmap")]
        {
            if let ReaderSource::Mmap(ref c) = self.source {
                Some(c.get_ref().as_ref())
            } else {
                None
            }
        }
        #[cfg(not(feature = "mmap"))]
        {
            None
        }
    }
}

impl Read for BddReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.source {
            ReaderSource::Seekable(s) => s.read(buf),
            ReaderSource::Streaming(s) => s.read(buf),
            #[cfg(feature = "mmap")]
            ReaderSource::Mmap(c) => c.read(buf),
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
            #[cfg(feature = "mmap")]
            ReaderSource::Mmap(c) => match c.seek(SeekFrom::Current(bytes as i64)) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            },
        }
    }

    fn rewind(&mut self) -> std::io::Result<bool> {
        match &mut self.source {
            ReaderSource::Seekable(s) => match s.seek(SeekFrom::Start(0)) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            },
            ReaderSource::Streaming(_) => Ok(false),
            #[cfg(feature = "mmap")]
            ReaderSource::Mmap(c) => match c.seek(SeekFrom::Start(0)) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            },
        }
    }
}

/// Opens a file as a RewindableBufRead, optionally memory mapping it for high-performance reading.
pub fn open_rewindable_file(file: std::fs::File, allow_mmap: bool) -> Box<dyn RewindableBufRead> {
    #[cfg(feature = "mmap")]
    if allow_mmap {
        if let Ok(meta) = file.metadata() {
            if meta.is_file() && meta.len() > 0 {
                if let Ok(mmap) = unsafe { memmap2::MmapOptions::new().map(&file) } {
                    #[cfg(unix)]
                    let _ = mmap.advise(memmap2::Advice::Sequential);
                    return Box::new(std::io::Cursor::new(mmap));
                }
            }
        }
    }
    let _ = allow_mmap;
    Box::new(BufReader::new(file))
}

impl<T: AsRef<[u8]>> StreamSeek for std::io::Cursor<T> {
    fn try_seek(&mut self, bytes: u64) -> std::io::Result<bool> {
        match self.seek(SeekFrom::Current(bytes as i64)) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    fn rewind(&mut self) -> std::io::Result<bool> {
        match self.seek(SeekFrom::Start(0)) {
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

    fn rewind(&mut self) -> std::io::Result<bool> {
        match self.seek(SeekFrom::Start(0)) {
            Ok(_) => Ok(true),
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

    fn rewind(&mut self) -> std::io::Result<bool> {
        match self.seek(SeekFrom::Start(0)) {
            Ok(_) => Ok(true),
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

    fn rewind(&mut self) -> std::io::Result<bool> {
        match self.seek(SeekFrom::Start(0)) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

/// Helper trait for buffered readers that can be rewound when repeating inputs.
pub trait RewindableBufRead: BufRead + StreamSeek {}
impl<T: BufRead + StreamSeek> RewindableBufRead for T {}

impl StreamSeek for Box<dyn RewindableBufRead> {
    fn try_seek(&mut self, bytes: u64) -> std::io::Result<bool> {
        (**self).try_seek(bytes)
    }

    fn rewind(&mut self) -> std::io::Result<bool> {
        (**self).rewind()
    }
}

/// Wrapper around BufReader for non-seekable readers that implements StreamSeek with no-ops.
pub struct StreamSeekBufReader<R>(pub BufReader<R>);

impl<R: Read> Read for StreamSeekBufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<R: Read> BufRead for StreamSeekBufReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.0.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.0.consume(amt)
    }
}

impl<R: Read> StreamSeek for StreamSeekBufReader<R> {
    fn try_seek(&mut self, _bytes: u64) -> std::io::Result<bool> {
        Ok(false)
    }

    fn rewind(&mut self) -> std::io::Result<bool> {
        Ok(false)
    }
}

/// Abstract iterator yielding units from an input source.
pub trait UnitStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError>;
    fn next_bit_value(&mut self) -> Result<Option<BitValue>, BddError> {
        match self.next_unit()? {
            Some(u) => Ok(Some(BitValue::from(u))),
            None => Ok(None),
        }
    }
    fn unit_size(&self) -> usize {
        8
    }
    fn read_bits(&mut self, bits: usize) -> Result<BigUint, BddError> {
        let mut acc = BigUint::zero();
        let mut rem = bits;
        while rem > 0 {
            let u_size = self.unit_size().min(rem);
            match self.next_unit()? {
                Some(u) => {
                    let mask = if u_size >= 64 {
                        (BigUint::one() << u_size) - 1u32
                    } else {
                        BigUint::from((1u64 << u_size) - 1)
                    };
                    acc = (acc << u_size) | (&u & &mask);
                    rem -= u_size;
                }
                None => break,
            }
        }
        Ok(acc)
    }
}

/// Wrapper around a UnitStream that optionally provides zeros infinitely upon EOF.
pub struct PaddedUnitStream {
    pub inner: Box<dyn UnitStream>,
    pub pad_zeros: bool,
}

impl PaddedUnitStream {
    pub fn new(inner: Box<dyn UnitStream>, pad_zeros: bool) -> Self {
        Self { inner, pad_zeros }
    }
}

impl UnitStream for PaddedUnitStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        match self.inner.next_unit()? {
            Some(u) => Ok(Some(u)),
            None => {
                if self.pad_zeros {
                    Ok(Some(BigUint::zero()))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn next_bit_value(&mut self) -> Result<Option<BitValue>, BddError> {
        match self.inner.next_bit_value()? {
            Some(u) => Ok(Some(u)),
            None => {
                if self.pad_zeros {
                    Ok(Some(BitValue::Inline(0)))
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn unit_size(&self) -> usize {
        self.inner.unit_size()
    }

    fn read_bits(&mut self, bits: usize) -> Result<BigUint, BddError> {
        self.inner.read_bits(bits)
    }
}

/// Bitstream reader extracting variable-width units from an underlying reader.
pub struct FileInputStream<R> {
    pub reader: R,
    pub buffer: BigUint,
    pub bits_in_buffer: usize,
    fast_buf: u128,
    read_buf: Box<[u8; 8192]>,
    read_pos: usize,
    read_len: usize,
    pub eof: bool,
    pub config: StreamConfig,
    pub counter: Counter,
    pub current_repeat: usize,
}

impl<R: Read + StreamSeek> FileInputStream<R> {
    pub fn new(reader: R, config: StreamConfig, counter: Counter) -> Self {
        Self {
            reader,
            buffer: BigUint::zero(),
            bits_in_buffer: 0,
            fast_buf: 0,
            read_buf: Box::new([0u8; 8192]),
            read_pos: 0,
            read_len: 0,
            eof: false,
            config,
            counter,
            current_repeat: 1,
        }
    }

    #[inline(always)]
    fn read_byte(&mut self) -> u8 {
        if self.read_pos < self.read_len {
            let mut val = self.read_buf[self.read_pos];
            self.read_pos += 1;
            if self.config.reverse_bytes {
                val = val.reverse_bits();
            }
            val
        } else {
            self.fill_and_read_byte()
        }
    }

    fn fill_and_read_byte(&mut self) -> u8 {
        match self.reader.read(&mut self.read_buf[..]) {
            Ok(n) if n > 0 => {
                self.read_len = n;
                self.read_pos = 1;
                let mut val = self.read_buf[0];
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
            let buffered = (self.read_len.saturating_sub(self.read_pos)) as u64;
            let mut remaining = if skip_bytes <= buffered {
                self.read_pos += skip_bytes as usize;
                0
            } else {
                self.read_pos = 0;
                self.read_len = 0;
                skip_bytes - buffered
            };
            if remaining > 0 && self.config.seek_allowed {
                if let Ok(true) = self.reader.try_seek(remaining) {
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
            let f_mask = (1u128 << self.bits_in_buffer) - 1;
            self.fast_buf = (b as u128) & f_mask;
        } else {
            let b = self.read_byte();
            self.buffer = BigUint::from(b);
            self.fast_buf = b as u128;
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
            let f_mask = if self.bits_in_buffer > 0 {
                if self.bits_in_buffer >= 128 {
                    !0u128
                } else {
                    (1u128 << self.bits_in_buffer) - 1
                }
            } else {
                0
            };
            self.fast_buf &= f_mask;
            return;
        }

        let remaining_gap = gap - (self.bits_in_buffer as u64);
        self.bits_in_buffer = 0;
        self.buffer = BigUint::zero();
        self.fast_buf = 0;

        let gap_bytes = remaining_gap / 8;
        let rem_bits = (remaining_gap % 8) as usize;

        if gap_bytes > 0 {
            let buffered = (self.read_len.saturating_sub(self.read_pos)) as u64;
            let mut remaining = if gap_bytes <= buffered {
                self.read_pos += gap_bytes as usize;
                0
            } else {
                self.read_pos = 0;
                self.read_len = 0;
                gap_bytes - buffered
            };
            if remaining > 0 && self.config.seek_allowed {
                if let Ok(true) = self.reader.try_seek(remaining) {
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
            let f_mask = (1u128 << self.bits_in_buffer) - 1;
            self.fast_buf = (b as u128) & f_mask;
        }
    }

    pub fn read_bits(&mut self, bits: usize) -> BigUint {
        if bits <= 64 {
            let val = self.read_bits_u64(bits);
            return BigUint::from(val);
        }
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
        if self.bits_in_buffer <= 64 {
            self.fast_buf = self.buffer.to_u64().unwrap_or(0) as u128;
        }
        unit
    }

    pub fn read_bits_u64(&mut self, bits: usize) -> u64 {
        while self.bits_in_buffer < bits {
            let b = self.read_byte();
            self.fast_buf = (self.fast_buf << 8) | (b as u128);
            self.bits_in_buffer += 8;
        }
        let right_edge = self.bits_in_buffer - bits;
        let mut unit = (self.fast_buf >> right_edge) as u64;
        let mask = if bits >= 64 {
            !0u64
        } else {
            (1u64 << bits) - 1
        };
        unit &= mask;
        if self.config.reverse_unit {
            unit = reverse_bits_u64(unit, self.config.unit_size);
        }
        self.bits_in_buffer -= bits;
        let rem_mask = if self.bits_in_buffer >= 128 {
            !0u128
        } else {
            (1u128 << self.bits_in_buffer) - 1
        };
        self.fast_buf &= rem_mask;
        self.buffer = BigUint::from(self.fast_buf as u64);
        unit
    }

    pub fn next_bit_value(&mut self) -> Result<Option<BitValue>, BddError> {
        if self.config.unit_size <= 64 {
            return self.next_bit_value_u64();
        }
        self.next_bit_value_bignum()
    }

    fn next_bit_value_u64(&mut self) -> Result<Option<BitValue>, BddError> {
        if self.eof {
            return Ok(None);
        }

        loop {
            if self.eof {
                if self.config.assert_aligned {
                    return Err(BddError::NonAlignedEof);
                }
                if !self.counter.finished()
                    && (self.config.repeat_count == 0
                        || self.current_repeat < self.config.repeat_count)
                    && self.reader.rewind()?
                {
                    self.current_repeat += 1;
                    self.eof = false;
                    self.read_pos = 0;
                    self.read_len = 0;
                    self.fast_buf = 0;
                    self.buffer = BigUint::zero();
                    self.bits_in_buffer = 0;
                    self.do_skip();
                    continue;
                }
                return Ok(None);
            }

            while self.bits_in_buffer < self.config.unit_size {
                let b = self.read_byte();
                if self.eof && self.config.drop_partial_eof {
                    self.bits_in_buffer = 0;
                    self.fast_buf = 0;
                    self.buffer = BigUint::zero();
                    if !self.counter.finished()
                        && (self.config.repeat_count == 0
                            || self.current_repeat < self.config.repeat_count)
                        && self.reader.rewind()?
                    {
                        self.current_repeat += 1;
                        self.eof = false;
                        self.read_pos = 0;
                        self.read_len = 0;
                        self.do_skip();
                        continue;
                    }
                    return Ok(None);
                }
                self.fast_buf = (self.fast_buf << 8) | (b as u128);
                self.bits_in_buffer += 8;
            }

            let right_edge = self.bits_in_buffer - self.config.unit_size;
            let mut unit = (self.fast_buf >> right_edge) as u64;
            let mask = if self.config.unit_size >= 64 {
                !0u64
            } else {
                (1u64 << self.config.unit_size) - 1
            };
            unit &= mask;

            self.counter.next();
            let included = self.counter.included();
            if self.counter.finished() {
                self.eof = true;
            }

            self.bits_in_buffer -= self.config.unit_size;
            let rem_mask = if self.bits_in_buffer >= 128 {
                !0u128
            } else {
                (1u128 << self.bits_in_buffer) - 1
            };
            self.fast_buf &= rem_mask;

            if self.bits_in_buffer == 0 && self.config.gap == 0 {
                let b = self.read_byte();
                if self.eof {
                    if included {
                        if self.config.reverse_unit {
                            unit = reverse_bits_u64(unit, self.config.unit_size);
                        }
                        if !self.counter.finished()
                            && (self.config.repeat_count == 0
                                || self.current_repeat < self.config.repeat_count)
                            && self.reader.rewind()?
                        {
                            self.current_repeat += 1;
                            self.eof = false;
                            self.read_pos = 0;
                            self.read_len = 0;
                            self.fast_buf = 0;
                            self.buffer = BigUint::zero();
                            self.bits_in_buffer = 0;
                            self.do_skip();
                        }
                        return Ok(Some(BitValue::Inline(unit)));
                    } else {
                        if !self.counter.finished()
                            && (self.config.repeat_count == 0
                                || self.current_repeat < self.config.repeat_count)
                            && self.reader.rewind()?
                        {
                            self.current_repeat += 1;
                            self.eof = false;
                            self.read_pos = 0;
                            self.read_len = 0;
                            self.fast_buf = 0;
                            self.buffer = BigUint::zero();
                            self.bits_in_buffer = 0;
                            self.do_skip();
                            continue;
                        }
                        return Ok(None);
                    }
                }
                self.fast_buf = b as u128;
                self.bits_in_buffer = 8;
            }

            if self.config.gap > 0 {
                self.skip_gap(self.config.gap);
            }

            if self.bits_in_buffer == 0 && !self.eof {
                let b = self.read_byte();
                if !self.eof {
                    self.fast_buf = b as u128;
                    self.bits_in_buffer = 8;
                }
            }

            if included {
                if self.config.reverse_unit {
                    unit = reverse_bits_u64(unit, self.config.unit_size);
                }
                return Ok(Some(BitValue::Inline(unit)));
            }
        }
    }

    fn next_bit_value_bignum(&mut self) -> Result<Option<BitValue>, BddError> {
        if self.eof {
            return Ok(None);
        }

        loop {
            if self.eof {
                if self.config.assert_aligned {
                    return Err(BddError::NonAlignedEof);
                }
                if !self.counter.finished()
                    && (self.config.repeat_count == 0
                        || self.current_repeat < self.config.repeat_count)
                    && self.reader.rewind()?
                {
                    self.current_repeat += 1;
                    self.eof = false;
                    self.read_pos = 0;
                    self.read_len = 0;
                    self.buffer = BigUint::zero();
                    self.bits_in_buffer = 0;
                    self.do_skip();
                    continue;
                }
                return Ok(None);
            }

            while self.bits_in_buffer < self.config.unit_size {
                let b = self.read_byte();
                if self.eof && self.config.drop_partial_eof {
                    self.bits_in_buffer = 0;
                    self.buffer = BigUint::zero();
                    if !self.counter.finished()
                        && (self.config.repeat_count == 0
                            || self.current_repeat < self.config.repeat_count)
                        && self.reader.rewind()?
                    {
                        self.current_repeat += 1;
                        self.eof = false;
                        self.read_pos = 0;
                        self.read_len = 0;
                        self.do_skip();
                        continue;
                    }
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
                        if !self.counter.finished()
                            && (self.config.repeat_count == 0
                                || self.current_repeat < self.config.repeat_count)
                            && self.reader.rewind()?
                        {
                            self.current_repeat += 1;
                            self.eof = false;
                            self.read_pos = 0;
                            self.read_len = 0;
                            self.buffer = BigUint::zero();
                            self.bits_in_buffer = 0;
                            self.do_skip();
                        }
                        return Ok(Some(BitValue::from(unit)));
                    } else {
                        if !self.counter.finished()
                            && (self.config.repeat_count == 0
                                || self.current_repeat < self.config.repeat_count)
                            && self.reader.rewind()?
                        {
                            self.current_repeat += 1;
                            self.eof = false;
                            self.read_pos = 0;
                            self.read_len = 0;
                            self.buffer = BigUint::zero();
                            self.bits_in_buffer = 0;
                            self.do_skip();
                            continue;
                        }
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
                return Ok(Some(BitValue::from(unit)));
            }
        }
    }
}

impl<R: Read + StreamSeek> UnitStream for FileInputStream<R> {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        self.next_bit_value()
            .map(|opt| opt.map(|bv| bv.into_biguint()))
    }

    fn next_bit_value(&mut self) -> Result<Option<BitValue>, BddError> {
        self.next_bit_value()
    }

    fn unit_size(&self) -> usize {
        self.config.unit_size
    }

    fn read_bits(&mut self, bits: usize) -> Result<BigUint, BddError> {
        Ok(self.read_bits(bits))
    }
}

pub struct ZeroStream {
    remaining: Option<u64>,
    pub unit_size: usize,
}

impl ZeroStream {
    pub fn new(counter: Counter) -> Self {
        Self {
            remaining: counter.count,
            unit_size: 8,
        }
    }

    pub fn new_with_unit(counter: Counter, unit_size: usize) -> Self {
        Self {
            remaining: counter.count,
            unit_size,
        }
    }
}

impl UnitStream for ZeroStream {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        self.next_bit_value()
            .map(|opt| opt.map(|bv| bv.into_biguint()))
    }

    fn next_bit_value(&mut self) -> Result<Option<BitValue>, BddError> {
        if let Some(rem) = &mut self.remaining {
            if *rem == 0 {
                return Ok(None);
            }
            *rem -= 1;
        }
        Ok(Some(BitValue::Inline(0)))
    }

    fn unit_size(&self) -> usize {
        self.unit_size
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
        self.next_bit_value()
            .map(|opt| opt.map(|bv| bv.into_biguint()))
    }

    fn next_bit_value(&mut self) -> Result<Option<BitValue>, BddError> {
        if let Some(rem) = &mut self.remaining {
            if *rem == 0 {
                return Ok(None);
            }
            *rem -= 1;
        }
        if self.unit_size <= 64 {
            let mask = if self.unit_size == 64 {
                !0u64
            } else {
                (1u64 << self.unit_size) - 1
            };
            Ok(Some(BitValue::Inline(mask)))
        } else {
            Ok(Some(BitValue::Big(
                (BigUint::one() << self.unit_size) - 1u32,
            )))
        }
    }

    fn unit_size(&self) -> usize {
        self.unit_size
    }
}

/// Stream generator producing cryptographically strong random bits sourced from `/dev/urandom`.
/// Employs a bit-level accumulator so every single bit of randomness is utilized without waste.
pub struct RandomStream {
    remaining: Option<u64>,
    pub unit_size: usize,
    urandom: Option<BufReader<File>>,
    bit_buffer: BigUint,
    bits_in_buffer: usize,
}

impl RandomStream {
    pub fn new(counter: Counter, unit_size: usize) -> Self {
        let urandom = File::open("/dev/urandom").ok().map(BufReader::new);
        let mut s = Self {
            remaining: counter.count,
            unit_size,
            urandom,
            bit_buffer: BigUint::zero(),
            bits_in_buffer: 0,
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
        if self.unit_size == 0 {
            return Ok(Some(BigUint::zero()));
        }

        while self.bits_in_buffer < self.unit_size {
            let needed_bits = self.unit_size - self.bits_in_buffer;
            let needed_bytes = needed_bits.div_ceil(8);
            let mut buf = vec![0u8; needed_bytes];
            if let Some(reader) = &mut self.urandom {
                reader.read_exact(&mut buf)?;
            } else {
                rand::rngs::OsRng.fill_bytes(&mut buf);
            }
            let chunk = BigUint::from_bytes_be(&buf);
            let chunk_bits = needed_bytes * 8;
            self.bit_buffer = (std::mem::take(&mut self.bit_buffer) << chunk_bits) | chunk;
            self.bits_in_buffer += chunk_bits;
        }

        let right_edge = self.bits_in_buffer - self.unit_size;
        let mut unit = &self.bit_buffer >> right_edge;
        self.bits_in_buffer -= self.unit_size;
        let mask = if self.bits_in_buffer > 0 {
            (BigUint::one() << self.bits_in_buffer) - 1u32
        } else {
            BigUint::zero()
        };
        self.bit_buffer &= mask;

        let unit_mask = (BigUint::one() << self.unit_size) - 1u32;
        unit &= unit_mask;
        Ok(Some(unit))
    }

    fn unit_size(&self) -> usize {
        self.unit_size
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
        self.next_bit_value()
            .map(|opt| opt.map(|bv| bv.into_biguint()))
    }

    fn next_bit_value(&mut self) -> Result<Option<BitValue>, BddError> {
        if let Some(rem) = &mut self.remaining {
            if *rem == 0 {
                return Ok(None);
            }
            *rem -= 1;
        }
        let val = self.current_val;
        self.current_val = self.current_val.wrapping_add(1);
        if self.unit_size <= 64 {
            let mask = if self.unit_size == 64 {
                !0u64
            } else {
                (1u64 << self.unit_size) - 1
            };
            Ok(Some(BitValue::Inline(val & mask)))
        } else {
            let mask = (BigUint::one() << self.unit_size) - 1u32;
            Ok(Some(BitValue::Big(BigUint::from(val) & mask)))
        }
    }

    fn unit_size(&self) -> usize {
        self.unit_size
    }
}

pub struct IntegerInputStream<R> {
    reader: R,
    counter: Counter,
    repeat_count: usize,
    current_repeat: usize,
}

impl<R: BufRead + StreamSeek> IntegerInputStream<R> {
    pub fn new(reader: R, counter: Counter, repeat_count: usize) -> Self {
        Self {
            reader,
            counter,
            repeat_count,
            current_repeat: 1,
        }
    }
}

impl<R: BufRead + StreamSeek> UnitStream for IntegerInputStream<R> {
    fn next_unit(&mut self) -> Result<Option<BigUint>, BddError> {
        let mut line = String::new();
        loop {
            line.clear();
            match self.reader.read_line(&mut line) {
                Ok(0) => {
                    if !self.counter.finished()
                        && (self.repeat_count == 0 || self.current_repeat < self.repeat_count)
                        && self.reader.rewind().unwrap_or(false)
                    {
                        self.current_repeat += 1;
                        continue;
                    }
                    return Ok(None);
                }
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
                    let bi = match parse_radix_bigint(trimmed) {
                        Some(i) => i,
                        None => {
                            crate::diag::warn(format!(
                                "Non-integer '{}' interpreted as zero",
                                trimmed
                            ));
                            BigInt::zero()
                        }
                    };
                    let val = if bi.is_negative() {
                        crate::diag::warn(format!(
                            "Negative integer '{}' interpreted as positive",
                            trimmed
                        ));
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
    repeat_count: usize,
    current_repeat: usize,
}

impl<R: BufRead + StreamSeek> TupleDirectInput<R> {
    pub fn new(reader: R, counter: Counter, repeat_count: usize) -> Self {
        Self {
            reader,
            counter,
            repeat_count,
            current_repeat: 1,
        }
    }

    pub fn next_tuple(&mut self) -> Result<Option<Vec<Field>>, BddError> {
        let mut raw_bytes = Vec::new();
        loop {
            raw_bytes.clear();
            match self.reader.read_until(b'\n', &mut raw_bytes) {
                Ok(0) => {
                    if !self.counter.finished()
                        && (self.repeat_count == 0 || self.current_repeat < self.repeat_count)
                        && self.reader.rewind().unwrap_or(false)
                    {
                        self.current_repeat += 1;
                        continue;
                    }
                    return Ok(None);
                }
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
                        } else if let Some(bi) = parse_radix_bigint(&field_str) {
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

    #[test]
    fn test_random_stream_urandom() {
        let mut stream = RandomStream::new(Counter::new(0, Some(5)), 12);
        assert_eq!(stream.unit_size, 12);
        let max_val = (BigUint::one() << 12) - 1u32;
        let mut count = 0;
        while let Some(unit) = stream.next_unit().unwrap() {
            assert!(unit <= max_val);
            count += 1;
        }
        assert_eq!(count, 5);
        assert_eq!(stream.next_unit().unwrap(), None);
    }

    #[test]
    fn test_random_stream_single_bit() {
        let mut stream = RandomStream::new(Counter::new(0, Some(24)), 1);
        assert_eq!(stream.unit_size, 1);
        let mut count = 0;
        while let Some(unit) = stream.next_unit().unwrap() {
            assert!(unit == BigUint::zero() || unit == BigUint::one());
            count += 1;
        }
        assert_eq!(count, 24);
        assert_eq!(stream.next_unit().unwrap(), None);
    }

    #[test]
    #[cfg(feature = "mmap")]
    fn test_bdd_reader_mmap() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"Hello Memory Mapped World!").unwrap();
        tmp.flush().unwrap();

        let file = File::open(tmp.path()).unwrap();
        let mut reader = BddReader::from_file_with_mmap(file, true, true);
        assert!(reader.is_mmap());
        assert!(reader.is_seekable());
        assert_eq!(reader.as_slice().unwrap(), b"Hello Memory Mapped World!");

        let mut buf = [0u8; 5];
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"Hello");

        assert!(reader.try_seek(1).unwrap()); // skip space
        let mut buf6 = [0u8; 6];
        reader.read_exact(&mut buf6).unwrap();
        assert_eq!(&buf6, b"Memory");

        assert!(reader.rewind().unwrap());
        reader.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"Hello");

        // Disabled mmap fallback test
        let file2 = File::open(tmp.path()).unwrap();
        let reader2 = BddReader::from_file_with_mmap(file2, true, false);
        assert!(!reader2.is_mmap());
        assert!(reader2.is_seekable());
    }

    #[test]
    #[cfg(feature = "mmap")]
    fn test_open_rewindable_file_mmap() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"123\n456\n").unwrap();
        tmp.flush().unwrap();

        let file = File::open(tmp.path()).unwrap();
        let mut reader = open_rewindable_file(file, true);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "123\n");
        assert!(reader.rewind().unwrap());
        line.clear();
        reader.read_line(&mut line).unwrap();
        assert_eq!(line, "123\n");
    }
}
