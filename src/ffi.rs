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
#[no_mangle]
pub extern "C" fn bdd_decode_f16(bits: u16) -> f64 {
    crate::float_types::decode_f16(bits)
}

/// Encodes a double float to an IEEE 754 half-precision float.
#[no_mangle]
pub extern "C" fn bdd_encode_f16(val: f64) -> u16 {
    crate::float_types::encode_f16(val)
}

/// Decodes a Bfloat16 float to double float.
#[no_mangle]
pub extern "C" fn bdd_decode_bf16(bits: u16) -> f64 {
    crate::float_types::decode_bf16(bits)
}

/// Encodes a double float to Bfloat16.
#[no_mangle]
pub extern "C" fn bdd_encode_bf16(val: f64) -> u16 {
    crate::float_types::encode_bf16(val)
}

/// Decodes an OCP FP8 E4M3FN float to double float.
#[no_mangle]
pub extern "C" fn bdd_decode_fp8_e4m3(bits: u8) -> f64 {
    crate::float_types::decode_fp8_e4m3(bits)
}

/// Encodes a double float to OCP FP8 E4M3FN.
#[no_mangle]
pub extern "C" fn bdd_encode_fp8_e4m3(val: f64) -> u8 {
    crate::float_types::encode_fp8_e4m3(val)
}

/// Decodes an NVIDIA Blackwell / OCP FP4 E2M1 float to double float.
#[no_mangle]
pub extern "C" fn bdd_decode_fp4_e2m1(bits: u8) -> f64 {
    crate::float_types::decode_fp4_e2m1(bits)
}

/// Encodes a double float to NVIDIA Blackwell / OCP FP4 E2M1.
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
