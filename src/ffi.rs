use crate::field::{reverse_bits, Field};
use crate::pattern::{TuplePacker, TupleUnpacker};
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

/// Reverse the lowest `bits` bits of a 64-bit integer.
#[no_mangle]
pub extern "C" fn bdd_reverse_bits_u64(val: u64, bits: usize) -> u64 {
    let big = reverse_bits(&BigUint::from(val), bits);
    big.to_u64().unwrap_or(0)
}

/// Decodes an IEEE 754 half-precision float to double float.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_decode_f16(bits: u16) -> f64 {
    crate::float_types::decode_f16(bits)
}

/// Encodes a double float to an IEEE 754 half-precision float.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_encode_f16(val: f64) -> u16 {
    crate::float_types::encode_f16(val)
}

/// Decodes a Bfloat16 float to double float.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_decode_bf16(bits: u16) -> f64 {
    crate::float_types::decode_bf16(bits)
}

/// Encodes a double float to Bfloat16.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_encode_bf16(val: f64) -> u16 {
    crate::float_types::encode_bf16(val)
}

/// Decodes an OCP FP8 E4M3FN float to double float.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_decode_fp8_e4m3(bits: u8) -> f64 {
    crate::float_types::decode_fp8_e4m3(bits)
}

/// Encodes a double float to OCP FP8 E4M3FN.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_encode_fp8_e4m3(val: f64) -> u8 {
    crate::float_types::encode_fp8_e4m3(val)
}

/// Decodes an OCP FP8 E5M2 float to double float.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_decode_fp8_e5m2(bits: u8) -> f64 {
    crate::float_types::decode_fp8_e5m2(bits)
}

/// Encodes a double float to OCP FP8 E5M2.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_encode_fp8_e5m2(val: f64) -> u8 {
    crate::float_types::encode_fp8_e5m2(val)
}

/// Decodes an OCP FP6 E3M2 float to double float.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_decode_fp6_e3m2(bits: u8) -> f64 {
    crate::float_types::decode_fp6_e3m2(bits)
}

/// Encodes a double float to OCP FP6 E3M2.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_encode_fp6_e3m2(val: f64) -> u8 {
    crate::float_types::encode_fp6_e3m2(val)
}

/// Decodes an NVIDIA Blackwell / OCP FP4 E2M1 float to double float.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_decode_fp4_e2m1(bits: u8) -> f64 {
    crate::float_types::decode_fp4_e2m1(bits)
}

/// Encodes a double float to NVIDIA Blackwell / OCP FP4 E2M1.
#[cfg(feature = "small-floats")]
#[no_mangle]
pub extern "C" fn bdd_encode_fp4_e2m1(val: f64) -> u8 {
    crate::float_types::encode_fp4_e2m1(val)
}

/// Unpack a 64-bit integer unit into an array of u64 field values.
/// Returns number of fields written, or negative error code.
///
/// # Safety
///
/// `pattern` must be a valid null-terminated C string.
/// `out_fields` must point to a writable buffer capable of holding at least `max_fields` `u64` values.
#[no_mangle]
pub unsafe extern "C" fn bdd_unpack_u64(
    pattern: *const c_char,
    unit: u64,
    out_fields: *mut u64,
    max_fields: usize,
) -> c_int {
    if pattern.is_null() || out_fields.is_null() {
        return -1;
    }
    let c_str = match CStr::from_ptr(pattern).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };
    let unpacker = match TupleUnpacker::new(c_str) {
        Ok(u) => u,
        Err(_) => return -3,
    };
    let tuple = unpacker.unpack(BigUint::from(unit));
    let n = tuple.len().min(max_fields);
    for (i, f) in tuple.iter().take(n).enumerate() {
        *out_fields.add(i) = f.as_biguint().to_u64().unwrap_or(0);
    }
    n as c_int
}

/// Pack an array of u64 field values into a 64-bit integer unit.
/// Returns 0 on success, negative error code on failure.
///
/// # Safety
///
/// `pattern` must be a valid null-terminated C string.
/// `fields` must point to a readable buffer of at least `num_fields` `u64` values.
/// `out_unit` must point to a writable `u64` variable.
#[no_mangle]
pub unsafe extern "C" fn bdd_pack_u64(
    pattern: *const c_char,
    fields: *const u64,
    num_fields: usize,
    out_unit: *mut u64,
) -> c_int {
    if pattern.is_null() || fields.is_null() || out_unit.is_null() {
        return -1;
    }
    let c_str = match CStr::from_ptr(pattern).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };
    let packer = match TuplePacker::new(c_str) {
        Ok(p) => p,
        Err(_) => return -3,
    };
    let mut tuple = Vec::with_capacity(num_fields);
    for i in 0..num_fields {
        let val = *fields.add(i);
        tuple.push(Field::UInt(BigUint::from(val)));
    }
    match packer.pack(tuple) {
        Ok(u) => {
            *out_unit = u.to_u64().unwrap_or(0);
            0
        }
        Err(_) => -4,
    }
}

/// Reads up to 64 bits from an in-memory buffer at an arbitrary non-aligned bit offset.
/// Returns 0 on success, negative error code on failure.
///
/// # Safety
/// `src` must point to a readable buffer of at least `src_len_bytes` bytes.
/// `out_val` must point to a writable `u64` variable.
#[no_mangle]
pub unsafe extern "C" fn bdd_read_bits_u64(
    src: *const u8,
    src_len_bytes: usize,
    bit_offset: usize,
    bit_count: usize,
    out_val: *mut u64,
) -> c_int {
    if src.is_null() || out_val.is_null() {
        return -1;
    }
    let slice = std::slice::from_raw_parts(src, src_len_bytes);
    match crate::bits::read_bits_u64(slice, bit_offset, bit_count) {
        Ok(v) => {
            *out_val = v;
            0
        }
        Err(_) => -2,
    }
}

/// Writes up to 64 bits into an in-memory buffer at an arbitrary non-aligned bit offset.
/// Modifies only the requested bits without altering surrounding bits in boundary bytes.
/// Returns 0 on success, negative error code on failure.
///
/// # Safety
/// `dst` must point to a writable buffer of at least `dst_len_bytes` bytes.
#[no_mangle]
pub unsafe extern "C" fn bdd_write_bits_u64(
    dst: *mut u8,
    dst_len_bytes: usize,
    bit_offset: usize,
    bit_count: usize,
    val: u64,
) -> c_int {
    if dst.is_null() {
        return -1;
    }
    let slice = std::slice::from_raw_parts_mut(dst, dst_len_bytes);
    match crate::bits::write_bits_u64(slice, bit_offset, bit_count, val) {
        Ok(()) => 0,
        Err(_) => -2,
    }
}

/// Copies `bit_count` bits from `src` (at `src_bit_offset`) to `dst` (at `dst_bit_offset`).
/// Source and destination can have completely independent, non-8-bit bit alignments.
/// Returns 0 on success, negative error code on failure.
///
/// # Safety
/// `src` must point to at least `src_len_bytes` readable bytes.
/// `dst` must point to at least `dst_len_bytes` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn bdd_copy_bits(
    src: *const u8,
    src_len_bytes: usize,
    src_bit_offset: usize,
    dst: *mut u8,
    dst_len_bytes: usize,
    dst_bit_offset: usize,
    bit_count: usize,
) -> c_int {
    if src.is_null() || dst.is_null() {
        return -1;
    }
    let s_slice = std::slice::from_raw_parts(src, src_len_bytes);
    let d_slice = std::slice::from_raw_parts_mut(dst, dst_len_bytes);
    match crate::bits::copy_bits(s_slice, src_bit_offset, d_slice, dst_bit_offset, bit_count) {
        Ok(()) => 0,
        Err(_) => -2,
    }
}

/// Unpacks structured tuple fields directly from an in-memory buffer starting at an unaligned bit offset.
/// Returns number of fields written, or negative error code on failure.
///
/// # Safety
/// `pattern` must be a valid null-terminated C string.
/// `src` must point to a readable buffer of at least `src_len_bytes` bytes.
/// `out_fields` must point to a writable buffer capable of holding at least `max_fields` `u64` values.
#[no_mangle]
pub unsafe extern "C" fn bdd_unpack_buffer(
    pattern: *const c_char,
    src: *const u8,
    src_len_bytes: usize,
    bit_offset: usize,
    out_fields: *mut u64,
    max_fields: usize,
) -> c_int {
    if pattern.is_null() || src.is_null() || out_fields.is_null() {
        return -1;
    }
    let c_str = match CStr::from_ptr(pattern).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };
    let unpacker = match TupleUnpacker::new(c_str) {
        Ok(u) => u,
        Err(_) => return -3,
    };
    let slice = std::slice::from_raw_parts(src, src_len_bytes);
    let unit = match crate::bits::read_bits_biguint(slice, bit_offset, unpacker.total_bits) {
        Ok(u) => u,
        Err(_) => return -4,
    };
    let tuple = unpacker.unpack(unit);
    let n = tuple.len().min(max_fields);
    for (i, f) in tuple.iter().take(n).enumerate() {
        *out_fields.add(i) = f.as_biguint().to_u64().unwrap_or(0);
    }
    n as c_int
}

/// Packs structured tuple fields directly into an in-memory buffer starting at an unaligned bit offset.
/// Returns number of bits written, or negative error code on failure.
///
/// # Safety
/// `pattern` must be a valid null-terminated C string.
/// `fields` must point to a readable buffer of at least `num_fields` `u64` values.
/// `dst` must point to a writable buffer of at least `dst_len_bytes` bytes.
#[no_mangle]
pub unsafe extern "C" fn bdd_pack_buffer(
    pattern: *const c_char,
    fields: *const u64,
    num_fields: usize,
    dst: *mut u8,
    dst_len_bytes: usize,
    bit_offset: usize,
) -> c_int {
    if pattern.is_null() || fields.is_null() || dst.is_null() {
        return -1;
    }
    let c_str = match CStr::from_ptr(pattern).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };
    let packer = match TuplePacker::new(c_str) {
        Ok(p) => p,
        Err(_) => return -3,
    };
    let mut tuple = Vec::with_capacity(num_fields);
    for i in 0..num_fields {
        let val = *fields.add(i);
        tuple.push(Field::UInt(BigUint::from(val)));
    }
    let unit = match packer.pack(tuple) {
        Ok(u) => u,
        Err(_) => return -4,
    };
    let slice = std::slice::from_raw_parts_mut(dst, dst_len_bytes);
    match crate::bits::write_bits_biguint(slice, bit_offset, packer.total_bits, &unit) {
        Ok(()) => packer.total_bits as c_int,
        Err(_) => -5,
    }
}

/// Returns the total bit width required by a pattern string, or negative on parse error.
///
/// # Safety
/// `pattern` must be a valid null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn bdd_pattern_total_bits(pattern: *const c_char) -> c_int {
    if pattern.is_null() {
        return -1;
    }
    let c_str = match CStr::from_ptr(pattern).to_str() {
        Ok(s) => s,
        Err(_) => return -2,
    };
    match TupleUnpacker::new(c_str) {
        Ok(u) => u.total_bits as c_int,
        Err(_) => -3,
    }
}
