use bdd::bits::{
    copy_bits, read_bits_u64, write_bits_u64,
    BitStreamReader, BitStreamWriter,
};

#[test]
fn test_unaligned_bitstream_rust_primitives() {
    // 1. Unaligned reads across multiple bytes
    let payload = [0x5A, 0xA5, 0xF0, 0x0F]; // 01011010 10100101 11110000 00001111
    // Offset 3, 11 bits:
    // Byte 0 bits 3..7: 11010 (5 bits)
    // Byte 1 bits 0..5: 101001 (6 bits)
    // Combined 11 bits: 11010_101001 = 0b11010101001 = 1705
    let val = read_bits_u64(&payload, 3, 11).unwrap();
    assert_eq!(val, 1705);

    // 2. Unaligned writes without disturbing neighbors
    let mut buffer = [0x00, 0x00, 0x00];
    write_bits_u64(&mut buffer, 5, 13, 0x1FFF).unwrap(); // 13 ones starting at bit 5
    // Bit 5..17 set to 1.
    // Byte 0: bits 5,6,7 = 0b0000_0111 = 0x07
    // Byte 1: bits 0..7  = 0b1111_1111 = 0xFF
    // Byte 2: bits 0,1   = 0b1100_0000 = 0xC0
    assert_eq!(buffer, [0x07, 0xFF, 0xC0]);
    assert_eq!(read_bits_u64(&buffer, 5, 13).unwrap(), 0x1FFF);

    // 3. Unaligned copy between different bit alignments
    let src = [0xDE, 0xAD, 0xBE, 0xEF];
    let mut dst = [0x00; 5];
    // Copy 21 bits from src[7] to dst[11]
    let src_val = read_bits_u64(&src, 7, 21).unwrap();
    copy_bits(&src, 7, &mut dst, 11, 21).unwrap();
    let dst_val = read_bits_u64(&dst, 11, 21).unwrap();
    assert_eq!(src_val, dst_val);
}

#[test]
fn test_bit_stream_reader_seeking_and_slicing() {
    let raw = [0x12, 0x34, 0x56, 0x78]; // 32 bits
    let mut reader = BitStreamReader::new(&raw);

    assert_eq!(reader.total_bits(), 32);
    assert_eq!(reader.remaining_bits(), 32);

    let b4 = reader.read_bits(4).unwrap();
    assert_eq!(b4, 0x1);
    assert_eq!(reader.bit_position(), 4);

    reader.skip_bits(4).unwrap();
    assert_eq!(reader.bit_position(), 8);

    let b8 = reader.read_bits(8).unwrap();
    assert_eq!(b8, 0x34);

    reader.seek_bit(20).unwrap();
    let b12 = reader.read_bits(12).unwrap();
    // bits 20..31: byte 2 bits 4..7 (0x6) and byte 3 bits 0..7 (0x78) => 0x678
    assert_eq!(b12, 0x678);
    assert!(reader.is_empty());

    // Slicing sub-window
    let sub = reader.slice_bits(0, 0).unwrap();
    assert_eq!(sub.remaining_bits(), 0);
}

#[test]
fn test_bit_stream_writer_accumulation() {
    let mut writer = BitStreamWriter::new();
    // 3-bit: 5 (101)
    writer.write_bits(5, 3).unwrap();
    // 7-bit: 100 (1100100)
    writer.write_bits(100, 7).unwrap();
    // 1-bit: 1 (1)
    writer.write_bits(1, 1).unwrap();

    assert_eq!(writer.total_bits(), 11);
    let (bytes, exact_bits) = writer.finish();
    assert_eq!(exact_bits, 11);
    assert_eq!(bytes.len(), 2);

    // Verify bit-exact reconstruction
    let mut reader = BitStreamReader::with_bit_bounds(&bytes, 0, 11).unwrap();
    assert_eq!(reader.read_bits(3).unwrap(), 5);
    assert_eq!(reader.read_bits(7).unwrap(), 100);
    assert_eq!(reader.read_bits(1).unwrap(), 1);
    assert!(reader.is_empty());
}

#[test]
fn test_ffi_non_aligned_functions() {
    use std::ffi::CString;

    let data = [0xAC, 0xF0]; // 10101100 11110000
    let mut val = 0u64;

    unsafe {
        let r = bdd::ffi::bdd_read_bits_u64(data.as_ptr(), data.len(), 6, 4, &mut val);
        assert_eq!(r, 0);
        assert_eq!(val, 3);

        let mut buf = [0xFF, 0x00];
        let r = bdd::ffi::bdd_write_bits_u64(buf.as_mut_ptr(), buf.len(), 6, 4, 3);
        assert_eq!(r, 0);
        assert_eq!(buf[0], 0xFC);
        assert_eq!(buf[1], 0xC0);

        let mut dst = [0x00; 3];
        let r = bdd::ffi::bdd_copy_bits(
            data.as_ptr(),
            data.len(),
            2,
            dst.as_mut_ptr(),
            dst.len(),
            5,
            8,
        );
        assert_eq!(r, 0);
        let mut read_back = 0u64;
        let mut expected = 0u64;
        bdd::ffi::bdd_read_bits_u64(data.as_ptr(), data.len(), 2, 8, &mut expected);
        bdd::ffi::bdd_read_bits_u64(dst.as_ptr(), dst.len(), 5, 8, &mut read_back);
        assert_eq!(read_back, expected);
        assert_eq!(read_back, 179);

        let pat = CString::new("3U2u3M").unwrap();
        let total = bdd::ffi::bdd_pattern_total_bits(pat.as_ptr());
        assert_eq!(total, 8);

        let mut fields = [0u64; 4];
        let test_tuple = [0b11110101u8];
        let n = bdd::ffi::bdd_unpack_buffer(
            pat.as_ptr(),
            test_tuple.as_ptr(),
            test_tuple.len(),
            0,
            fields.as_mut_ptr(),
            4,
        );
        assert_eq!(n, 4);
    }
}
