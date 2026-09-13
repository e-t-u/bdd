//! In-memory non-8-bit-aligned bitstream primitives, slicing, packing, and streaming.
//!
//! Provides zero-copy reading, bit-exact mutation, unaligned bit copying,
//! and ergonomic [`BitStreamReader`] and [`BitStreamWriter`] abstractions for arbitrary-width
//! bit manipulation in memory.

use crate::error::BddError;
use crate::field::Field;
use crate::pattern::{TuplePacker, TupleUnpacker};
use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};

/// Reads up to 64 bits from a byte slice at an arbitrary non-8-bit-aligned bit offset.
///
/// Bits are indexed in standard network/big-endian bit order:
/// bit 0 is MSB of byte 0 (`0x80`), bit 7 is LSB of byte 0 (`0x01`),
/// bit 8 is MSB of byte 1 (`0x80`), etc.
///
/// # Errors
/// Returns [`BddError::InvalidBitWidth`] if `bit_count > 64`.
/// Returns [`BddError::OutOfBounds`] if `bit_offset + bit_count` exceeds available bits in `src`.
pub fn read_bits_u64(src: &[u8], bit_offset: usize, bit_count: usize) -> Result<u64, BddError> {
    if bit_count == 0 {
        return Ok(0);
    }
    if bit_count > 64 {
        return Err(BddError::InvalidBitWidth(bit_count));
    }
    let total_src_bits = src.len().saturating_mul(8);
    if bit_offset
        .checked_add(bit_count)
        .is_none_or(|end| end > total_src_bits)
    {
        return Err(BddError::OutOfBounds(format!(
            "read_bits_u64: offset {} + count {} exceeds buffer bit length {}",
            bit_offset, bit_count, total_src_bits
        )));
    }

    let start_byte = bit_offset / 8;
    let bit_in_byte = bit_offset % 8;
    let end_bit = bit_offset + bit_count - 1;
    let end_byte = end_bit / 8;
    let num_bytes = end_byte - start_byte + 1;

    let mut raw = 0u128;
    for i in 0..num_bytes {
        raw = (raw << 8) | (src[start_byte + i] as u128);
    }

    let total_bits = num_bytes * 8;
    let shift_right = total_bits - bit_in_byte - bit_count;
    let val = (raw >> shift_right) as u64;
    let mask = if bit_count == 64 {
        !0u64
    } else {
        (1u64 << bit_count) - 1
    };
    Ok(val & mask)
}

/// Writes up to 64 bits into a mutable byte slice at an arbitrary non-8-bit-aligned bit offset.
///
/// Only the targeted `bit_count` bits are modified; all surrounding bits in boundary bytes
/// remain completely untouched.
///
/// # Errors
/// Returns [`BddError::InvalidBitWidth`] if `bit_count > 64`.
/// Returns [`BddError::OutOfBounds`] if `bit_offset + bit_count` exceeds available bits in `dst`.
pub fn write_bits_u64(
    dst: &mut [u8],
    bit_offset: usize,
    bit_count: usize,
    val: u64,
) -> Result<(), BddError> {
    if bit_count == 0 {
        return Ok(());
    }
    if bit_count > 64 {
        return Err(BddError::InvalidBitWidth(bit_count));
    }
    let total_dst_bits = dst.len().saturating_mul(8);
    if bit_offset
        .checked_add(bit_count)
        .is_none_or(|end| end > total_dst_bits)
    {
        return Err(BddError::OutOfBounds(format!(
            "write_bits_u64: offset {} + count {} exceeds buffer bit length {}",
            bit_offset, bit_count, total_dst_bits
        )));
    }

    let start_byte = bit_offset / 8;
    let bit_in_byte = bit_offset % 8;
    let end_bit = bit_offset + bit_count - 1;
    let end_byte = end_bit / 8;
    let num_bytes = end_byte - start_byte + 1;

    let val_masked = if bit_count == 64 {
        val
    } else {
        val & ((1u64 << bit_count) - 1)
    };
    let total_bits = num_bytes * 8;
    let shift_right = total_bits - bit_in_byte - bit_count;

    let mut existing = 0u128;
    for i in 0..num_bytes {
        existing = (existing << 8) | (dst[start_byte + i] as u128);
    }

    let mask_chunk = if bit_count == 128 {
        !0u128
    } else {
        (1u128 << bit_count) - 1
    };
    let mask_128 = mask_chunk << shift_right;
    let val_shifted = (val_masked as u128) << shift_right;

    let updated = (existing & !mask_128) | (val_shifted & mask_128);
    for i in 0..num_bytes {
        let shift = (num_bytes - 1 - i) * 8;
        dst[start_byte + i] = (updated >> shift) as u8;
    }
    Ok(())
}

/// Reads an arbitrary number of bits (including > 64 bits) into a [`BigUint`].
pub fn read_bits_biguint(
    src: &[u8],
    bit_offset: usize,
    bit_count: usize,
) -> Result<BigUint, BddError> {
    if bit_count <= 64 {
        return read_bits_u64(src, bit_offset, bit_count).map(BigUint::from);
    }
    let total_src_bits = src.len().saturating_mul(8);
    if bit_offset
        .checked_add(bit_count)
        .is_none_or(|end| end > total_src_bits)
    {
        return Err(BddError::OutOfBounds(format!(
            "read_bits_biguint: offset {} + count {} exceeds buffer bit length {}",
            bit_offset, bit_count, total_src_bits
        )));
    }

    let mut result = BigUint::zero();
    let mut remaining = bit_count;
    let mut curr_offset = bit_offset;
    while remaining > 0 {
        let chunk = remaining.min(64);
        let u = read_bits_u64(src, curr_offset, chunk)?;
        result = (result << chunk) | BigUint::from(u);
        curr_offset += chunk;
        remaining -= chunk;
    }
    Ok(result)
}

/// Writes an arbitrary number of bits (including > 64 bits) from a [`BigUint`].
pub fn write_bits_biguint(
    dst: &mut [u8],
    bit_offset: usize,
    bit_count: usize,
    val: &BigUint,
) -> Result<(), BddError> {
    if bit_count <= 64 {
        return write_bits_u64(dst, bit_offset, bit_count, val.to_u64().unwrap_or(0));
    }
    let total_dst_bits = dst.len().saturating_mul(8);
    if bit_offset
        .checked_add(bit_count)
        .is_none_or(|end| end > total_dst_bits)
    {
        return Err(BddError::OutOfBounds(format!(
            "write_bits_biguint: offset {} + count {} exceeds buffer bit length {}",
            bit_offset, bit_count, total_dst_bits
        )));
    }

    let mut remaining = bit_count;
    let mut curr_offset = bit_offset;
    while remaining > 0 {
        let chunk = remaining.min(64);
        let shift = remaining - chunk;
        let mask = if chunk == 64 {
            !0u64
        } else {
            (1u64 << chunk) - 1
        };
        let chunk_val = ((val >> shift) & BigUint::from(mask)).to_u64().unwrap_or(0);
        write_bits_u64(dst, curr_offset, chunk, chunk_val)?;
        curr_offset += chunk;
        remaining -= chunk;
    }
    Ok(())
}

/// Copies `bit_count` bits from `src` (starting at `src_bit_offset`) to `dst` (starting at `dst_bit_offset`).
///
/// Source and destination offsets can have completely independent, non-8-bit bit alignments.
pub fn copy_bits(
    src: &[u8],
    src_bit_offset: usize,
    dst: &mut [u8],
    dst_bit_offset: usize,
    bit_count: usize,
) -> Result<(), BddError> {
    let mut remaining = bit_count;
    let mut s_off = src_bit_offset;
    let mut d_off = dst_bit_offset;
    while remaining > 0 {
        let chunk = remaining.min(64);
        let val = read_bits_u64(src, s_off, chunk)?;
        write_bits_u64(dst, d_off, chunk, val)?;
        s_off += chunk;
        d_off += chunk;
        remaining -= chunk;
    }
    Ok(())
}

/// Ergonomic zero-allocation in-memory reader for arbitrary-width and unaligned bitstreams.
#[derive(Debug, Clone)]
pub struct BitStreamReader<'a> {
    data: &'a [u8],
    bit_pos: usize,
    bit_len: usize,
}

impl<'a> BitStreamReader<'a> {
    /// Creates a new reader over a byte slice, reading all available bits (`data.len() * 8`).
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            bit_pos: 0,
            bit_len: data.len().saturating_mul(8),
        }
    }

    /// Creates a new reader restricted to an explicit bit window `[bit_offset..bit_offset + bit_len]`.
    pub fn with_bit_bounds(
        data: &'a [u8],
        bit_offset: usize,
        bit_len: usize,
    ) -> Result<Self, BddError> {
        let total = data.len().saturating_mul(8);
        if bit_offset
            .checked_add(bit_len)
            .is_none_or(|end| end > total)
        {
            return Err(BddError::OutOfBounds(format!(
                "BitStreamReader: window [{}..{}] exceeds buffer size {}",
                bit_offset,
                bit_offset + bit_len,
                total
            )));
        }
        Ok(Self {
            data,
            bit_pos: bit_offset,
            bit_len: bit_offset + bit_len,
        })
    }

    /// Returns the current bit cursor position relative to the underlying buffer.
    pub fn bit_position(&self) -> usize {
        self.bit_pos
    }

    /// Returns the total bit length of this reader stream.
    pub fn total_bits(&self) -> usize {
        self.bit_len
    }

    /// Returns the number of unconsumed bits remaining.
    pub fn remaining_bits(&self) -> usize {
        self.bit_len.saturating_sub(self.bit_pos)
    }

    /// Returns true if all bits in the stream have been consumed.
    pub fn is_empty(&self) -> bool {
        self.remaining_bits() == 0
    }

    /// Seeks the bit cursor to an absolute bit position in the stream.
    pub fn seek_bit(&mut self, bit_pos: usize) -> Result<(), BddError> {
        if bit_pos > self.bit_len {
            return Err(BddError::OutOfBounds(format!(
                "seek_bit: target position {} exceeds stream limit {}",
                bit_pos, self.bit_len
            )));
        }
        self.bit_pos = bit_pos;
        Ok(())
    }

    /// Skips forward by `n` bits.
    pub fn skip_bits(&mut self, n: usize) -> Result<(), BddError> {
        if n > self.remaining_bits() {
            return Err(BddError::OutOfBounds(format!(
                "skip_bits: skip amount {} exceeds remaining bits {}",
                n,
                self.remaining_bits()
            )));
        }
        self.bit_pos += n;
        Ok(())
    }

    /// Reads up to 64 bits and advances the bit cursor.
    pub fn read_bits(&mut self, n: usize) -> Result<u64, BddError> {
        if n > self.remaining_bits() {
            return Err(BddError::OutOfBounds(format!(
                "read_bits: requested {} bits but only {} remaining",
                n,
                self.remaining_bits()
            )));
        }
        let val = read_bits_u64(self.data, self.bit_pos, n)?;
        self.bit_pos += n;
        Ok(val)
    }

    /// Reads an arbitrary number of bits into a [`BigUint`] and advances the bit cursor.
    pub fn read_biguint(&mut self, n: usize) -> Result<BigUint, BddError> {
        if n > self.remaining_bits() {
            return Err(BddError::OutOfBounds(format!(
                "read_biguint: requested {} bits but only {} remaining",
                n,
                self.remaining_bits()
            )));
        }
        let val = read_bits_biguint(self.data, self.bit_pos, n)?;
        self.bit_pos += n;
        Ok(val)
    }

    /// Reads structured fields matching a [`TupleUnpacker`] from the current bit position.
    pub fn read_tuple(&mut self, unpacker: &TupleUnpacker) -> Result<Vec<Field>, BddError> {
        let n = unpacker.total_bits;
        if n > self.remaining_bits() {
            return Err(BddError::OutOfBounds(format!(
                "read_tuple: pattern requires {} bits but only {} remaining",
                n,
                self.remaining_bits()
            )));
        }
        let unit = self.read_biguint(n)?;
        Ok(unpacker.unpack(unit))
    }

    /// Slices a sub-region of bits from the current position without copying memory.
    pub fn slice_bits(&self, offset: usize, count: usize) -> Result<BitStreamReader<'a>, BddError> {
        let start = self.bit_pos.checked_add(offset).ok_or_else(|| {
            BddError::OutOfBounds("slice_bits: offset calculation overflow".to_string())
        })?;
        if start
            .checked_add(count)
            .is_none_or(|end| end > self.bit_len)
        {
            return Err(BddError::OutOfBounds(format!(
                "slice_bits: slice [{}..{}] exceeds limit {}",
                start,
                start + count,
                self.bit_len
            )));
        }
        Ok(BitStreamReader {
            data: self.data,
            bit_pos: start,
            bit_len: start + count,
        })
    }
}

/// Ergonomic bitstream writer that accumulates arbitrary-width bitfields into packed bytes.
#[derive(Debug, Clone, Default)]
pub struct BitStreamWriter {
    buffer: Vec<u8>,
    total_bits: usize,
}

impl BitStreamWriter {
    /// Creates a new empty bitstream writer.
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            total_bits: 0,
        }
    }

    /// Creates a new writer with pre-allocated buffer capacity in bits.
    pub fn with_capacity_bits(bits: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(bits.div_ceil(8)),
            total_bits: 0,
        }
    }

    /// Returns the exact total number of bits written so far (may be non-8-bit aligned).
    pub fn total_bits(&self) -> usize {
        self.total_bits
    }

    /// Writes up to 64 bits to the stream.
    pub fn write_bits(&mut self, val: u64, n: usize) -> Result<(), BddError> {
        if n == 0 {
            return Ok(());
        }
        let needed_bits = self.total_bits.checked_add(n).ok_or_else(|| {
            BddError::OutOfBounds("BitStreamWriter: bit overflow".to_string())
        })?;
        let needed_bytes = needed_bits.div_ceil(8);
        if needed_bytes > self.buffer.len() {
            self.buffer.resize(needed_bytes, 0);
        }
        write_bits_u64(&mut self.buffer, self.total_bits, n, val)?;
        self.total_bits = needed_bits;
        Ok(())
    }

    /// Writes arbitrary-width bits from a [`BigUint`] to the stream.
    pub fn write_biguint(&mut self, val: &BigUint, n: usize) -> Result<(), BddError> {
        if n == 0 {
            return Ok(());
        }
        let needed_bits = self.total_bits.checked_add(n).ok_or_else(|| {
            BddError::OutOfBounds("BitStreamWriter: bit overflow".to_string())
        })?;
        let needed_bytes = needed_bits.div_ceil(8);
        if needed_bytes > self.buffer.len() {
            self.buffer.resize(needed_bytes, 0);
        }
        write_bits_biguint(&mut self.buffer, self.total_bits, n, val)?;
        self.total_bits = needed_bits;
        Ok(())
    }

    /// Packs structured tuple fields into the stream according to a [`TuplePacker`].
    pub fn write_tuple(
        &mut self,
        packer: &TuplePacker,
        fields: Vec<Field>,
    ) -> Result<(), BddError> {
        let unit = packer.pack(fields)?;
        self.write_biguint(&unit, packer.total_bits)
    }

    /// Borrows the underlying byte slice. Note: the final byte may contain trailing padding bits.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer
    }

    /// Consumes the writer and returns the packed bytes (final byte zero-padded to boundary).
    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }

    /// Consumes the writer and returns both the packed bytes and the exact bit length.
    pub fn finish(self) -> (Vec<u8>, usize) {
        let bits = self.total_bits;
        (self.buffer, bits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_traits::One;

    #[test]
    fn test_read_bits_aligned_and_unaligned() {
        let data = [0b1010_1100, 0b1111_0000]; // 0xAC, 0xF0

        // Bit 0..3: 1010 = 10
        assert_eq!(read_bits_u64(&data, 0, 4).unwrap(), 10);
        // Bit 2..6: 1011 = 11
        assert_eq!(read_bits_u64(&data, 2, 4).unwrap(), 11);
        // Cross byte boundary: bit 6..10 (2 bits from byte 0, 2 bits from byte 1):
        // byte 0 bit 6,7: 0, 0; byte 1 bit 0,1: 1, 1 => 0011 = 3
        assert_eq!(read_bits_u64(&data, 6, 4).unwrap(), 3);
        // Bit 8..16: byte 1 = 0xF0 = 240
        assert_eq!(read_bits_u64(&data, 8, 8).unwrap(), 0xF0);
        // Zero count
        assert_eq!(read_bits_u64(&data, 4, 0).unwrap(), 0);
        // Out of bounds
        assert!(read_bits_u64(&data, 10, 8).is_err());
    }

    #[test]
    fn test_write_bits_preserves_neighbors() {
        let mut buf = [0xFF, 0x00];

        // Write 4 bits = 3 (0b0011) at offset 6 (crossing boundary)
        write_bits_u64(&mut buf, 6, 4, 3).unwrap();
        // Byte 0 bits 0..5 untouched (111111 = 0xFC), bits 6..7 = 00
        assert_eq!(buf[0], 0xFC);
        // Byte 1 bits 0..1 = 11, bits 2..7 untouched (000000) => 0xC0
        assert_eq!(buf[1], 0xC0);

        // Read back
        assert_eq!(read_bits_u64(&buf, 6, 4).unwrap(), 3);
    }

    #[test]
    fn test_copy_bits_unaligned() {
        let src = [0b1101_0010, 0b1011_0101];
        let mut dst = [0x00, 0x00, 0x00];

        // Copy 11 bits from src[3] to dst[5]
        let val = read_bits_u64(&src, 3, 11).unwrap();
        copy_bits(&src, 3, &mut dst, 5, 11).unwrap();
        let read_back = read_bits_u64(&dst, 5, 11).unwrap();
        assert_eq!(val, read_back);
    }

    #[test]
    fn test_bit_stream_reader_and_writer() {
        let mut writer = BitStreamWriter::new();
        // Write 3-bit unit: 5 (101)
        writer.write_bits(5, 3).unwrap();
        // Write 12-bit unit: 0xABC
        writer.write_bits(0xABC, 12).unwrap();
        // Write 1-bit flag: 1
        writer.write_bits(1, 1).unwrap();

        assert_eq!(writer.total_bits(), 16);
        let (bytes, total_bits) = writer.finish();
        assert_eq!(total_bits, 16);
        assert_eq!(bytes.len(), 2);

        let mut reader = BitStreamReader::new(&bytes);
        assert_eq!(reader.remaining_bits(), 16);
        assert_eq!(reader.read_bits(3).unwrap(), 5);
        assert_eq!(reader.read_bits(12).unwrap(), 0xABC);
        assert_eq!(reader.read_bits(1).unwrap(), 1);
        assert_eq!(reader.remaining_bits(), 0);
        assert!(reader.is_empty());
    }

    #[test]
    fn test_biguint_bits() {
        let mut buf = vec![0u8; 32]; // 256 bits
        let big = (BigUint::one() << 100) | BigUint::from(0xDEADBEEFu64);
        write_bits_biguint(&mut buf, 10, 105, &big).unwrap();
        let read_back = read_bits_biguint(&buf, 10, 105).unwrap();
        assert_eq!(big, read_back);
    }

    #[test]
    fn test_tuple_reader_writer() {
        let unpacker = TupleUnpacker::new("3U2u3M").unwrap();
        let packer = TuplePacker::new("3U2u3M").unwrap();

        // 8 bits total: 3U (3 bits) + 2u (2 bits) + 3M (3 bits)
        let mut writer = BitStreamWriter::new();
        let fields = vec![
            Field::UInt(BigUint::from(7u32)),        // 3U
            Field::UInt(BigUint::from(2u32)),        // 2u
            Field::UInt(BigUint::from(1u32)),        // 3M sign
            Field::UInt(BigUint::from(1u32)),        // 3M mag
        ];
        writer.write_tuple(&packer, fields).unwrap();
        assert_eq!(writer.total_bits(), 8);

        let (bytes, bits) = writer.finish();
        assert_eq!(bits, 8);
        assert_eq!(bytes.len(), 1);

        let mut reader = BitStreamReader::new(&bytes);
        let unpacked = reader.read_tuple(&unpacker).unwrap();
        assert_eq!(unpacked.len(), 4); // 3U, 2u, 3M sign, 3M mag
        assert_eq!(unpacked[0], Field::UInt(BigUint::from(7u32)));
        assert_eq!(unpacked[1], Field::UInt(BigUint::from(2u32))); // 2u roundtripped
        assert_eq!(unpacked[2], Field::UInt(BigUint::from(1u32))); // sign
        assert_eq!(unpacked[3], Field::UInt(BigUint::from(1u32))); // mag
    }
}
