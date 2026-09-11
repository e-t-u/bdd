//! Conversion routines for AI and GPU floating point formats:
//! - IEEE 754 Half-precision FP16 (16H / 16h)
//! - Google Brain Bfloat16 BF16 (16Y / 16y)
//! - OCP FP8 E4M3FN (8E / 8e)
//! - OCP FP8 E5M2 (8Q / 8q)
//! - OCP FP6 E3M2 (6E / 6e)
//! - NVIDIA Blackwell / OCP FP4 E2M1 (4E / 4e)

/// Decodes an IEEE 754 half-precision (16-bit) float to f64.
pub fn decode_f16(bits: u16) -> f64 {
    let sign = (bits >> 15) & 1;
    let exp = (bits >> 10) & 0x1F;
    let mant = bits & 0x3FF;

    let sign_mul = if sign == 1 { -1.0 } else { 1.0 };

    if exp == 0 {
        if mant == 0 {
            0.0 * sign_mul
        } else {
            // Subnormal: 2^(-14) * (mant / 1024)
            sign_mul * (mant as f64 / 1024.0) * (2.0f64).powi(-14)
        }
    } else if exp == 31 {
        if mant == 0 {
            if sign == 1 {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }
        } else {
            f64::NAN
        }
    } else {
        // Normal: 2^(exp - 15) * (1 + mant / 1024)
        sign_mul * (1.0 + mant as f64 / 1024.0) * (2.0f64).powi(exp as i32 - 15)
    }
}

/// Encodes an f64 to an IEEE 754 half-precision (16-bit) float.
pub fn encode_f16(val: f64) -> u16 {
    if val.is_nan() {
        return 0x7E00; // Standard qNaN
    }
    let sign_bit = if val.is_sign_negative() {
        1u16 << 15
    } else {
        0
    };
    if val.is_infinite() {
        return sign_bit | 0x7C00;
    }
    if val == 0.0 {
        return sign_bit;
    }

    let abs_val = val.abs();
    // Clamping / overflow
    if abs_val >= 65520.0 {
        return sign_bit | 0x7C00; // Infinity
    }
    // Subnormal threshold: 2^(-14) = ~6.1035e-5
    if abs_val < (2.0f64).powi(-14) {
        let mant = (abs_val / (2.0f64).powi(-24)).round() as u16;
        return sign_bit | (mant & 0x3FF);
    }

    // Normal numbers
    let mut exp = (abs_val.log2().floor()) as i32;
    let mant_frac = abs_val / (2.0f64).powi(exp) - 1.0;
    let mut mant = (mant_frac * 1024.0).round() as u16;
    if mant >= 1024 {
        exp += 1;
        mant = 0;
    }
    let biased_exp = ((exp + 15).clamp(0, 31)) as u16;
    sign_bit | (biased_exp << 10) | (mant & 0x3FF)
}

/// Decodes a Bfloat16 (16-bit) float to f64.
pub fn decode_bf16(bits: u16) -> f64 {
    let u32_val = (bits as u32) << 16;
    f32::from_bits(u32_val) as f64
}

/// Encodes an f64 to Bfloat16 (16-bit).
pub fn encode_bf16(val: f64) -> u16 {
    let f = val as f32;
    let u = f.to_bits();
    // Round to nearest even
    let rounding_bias = 0x7FFF + ((u >> 16) & 1);
    ((u.wrapping_add(rounding_bias)) >> 16) as u16
}

/// Decodes an OCP FP8 E4M3FN (8-bit) float to f64.
/// Layout: 1 sign, 4 exponent, 3 mantissa, bias 7.
pub fn decode_fp8_e4m3(bits: u8) -> f64 {
    let sign = (bits >> 7) & 1;
    let exp = (bits >> 3) & 0x0F;
    let mant = bits & 0x07;

    let sign_mul = if sign == 1 { -1.0 } else { 1.0 };

    if exp == 0 {
        if mant == 0 {
            0.0 * sign_mul
        } else {
            // Subnormal: 2^(-6) * (mant / 8)
            sign_mul * (mant as f64 / 8.0) * (2.0f64).powi(-6)
        }
    } else if exp == 15 && mant == 7 {
        // E4M3FN NaN representation
        f64::NAN
    } else {
        // Normal: 2^(exp - 7) * (1 + mant / 8)
        sign_mul * (1.0 + mant as f64 / 8.0) * (2.0f64).powi(exp as i32 - 7)
    }
}

/// Encodes an f64 to OCP FP8 E4M3FN.
pub fn encode_fp8_e4m3(val: f64) -> u8 {
    if val.is_nan() {
        return 0x7F; // NaN in E4M3FN
    }
    let sign_bit = if val.is_sign_negative() { 1u8 << 7 } else { 0 };
    let abs_val = val.abs();
    if abs_val == 0.0 {
        return sign_bit;
    }
    // Max finite value: exp=15, mant=6 -> 448.0
    if abs_val >= 448.0 {
        return sign_bit | 0x7E;
    }
    // Subnormal threshold: 2^(-6) = 0.015625
    if abs_val < (2.0f64).powi(-6) {
        let mant = (abs_val / (2.0f64).powi(-9)).round() as u8;
        return sign_bit | (mant & 0x07);
    }
    let mut exp = (abs_val.log2().floor()) as i32;
    let mant_frac = abs_val / (2.0f64).powi(exp) - 1.0;
    let mut mant = (mant_frac * 8.0).round() as u8;
    if mant >= 8 {
        exp += 1;
        mant = 0;
    }
    let biased_exp = ((exp + 7).clamp(0, 15)) as u8;
    if biased_exp == 15 && mant >= 7 {
        mant = 6;
    }
    sign_bit | (biased_exp << 3) | (mant & 0x07)
}

/// Decodes an OCP FP8 E5M2 (8-bit) float to f64.
/// Layout: 1 sign, 5 exponent, 2 mantissa, bias 15.
pub fn decode_fp8_e5m2(bits: u8) -> f64 {
    let sign = (bits >> 7) & 1;
    let exp = (bits >> 2) & 0x1F;
    let mant = bits & 0x03;

    let sign_mul = if sign == 1 { -1.0 } else { 1.0 };

    if exp == 0 {
        if mant == 0 {
            0.0 * sign_mul
        } else {
            // Subnormal: 2^(-14) * (mant / 4)
            sign_mul * (mant as f64 / 4.0) * (2.0f64).powi(-14)
        }
    } else if exp == 31 {
        if mant == 0 {
            if sign == 1 {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            }
        } else {
            f64::NAN
        }
    } else {
        // Normal: 2^(exp - 15) * (1 + mant / 4)
        sign_mul * (1.0 + mant as f64 / 4.0) * (2.0f64).powi(exp as i32 - 15)
    }
}

/// Encodes an f64 to OCP FP8 E5M2.
pub fn encode_fp8_e5m2(val: f64) -> u8 {
    if val.is_nan() {
        return 0x7F;
    }
    let sign_bit = if val.is_sign_negative() { 1u8 << 7 } else { 0 };
    if val.is_infinite() {
        return sign_bit | 0x7C;
    }
    let abs_val = val.abs();
    if abs_val == 0.0 {
        return sign_bit;
    }
    if abs_val >= 57344.0 {
        return sign_bit | 0x7C;
    }
    if abs_val < (2.0f64).powi(-14) {
        let mant = (abs_val / (2.0f64).powi(-16)).round() as u8;
        return sign_bit | (mant & 0x03);
    }
    let mut exp = (abs_val.log2().floor()) as i32;
    let mant_frac = abs_val / (2.0f64).powi(exp) - 1.0;
    let mut mant = (mant_frac * 4.0).round() as u8;
    if mant >= 4 {
        exp += 1;
        mant = 0;
    }
    let biased_exp = ((exp + 15).clamp(0, 31)) as u8;
    sign_bit | (biased_exp << 2) | (mant & 0x03)
}

/// Decodes an OCP FP6 E3M2 (6-bit) float to f64.
/// Layout: 1 sign, 3 exponent, 2 mantissa, bias 3.
pub fn decode_fp6_e3m2(bits: u8) -> f64 {
    let sign = (bits >> 5) & 1;
    let exp = (bits >> 2) & 0x07;
    let mant = bits & 0x03;

    let sign_mul = if sign == 1 { -1.0 } else { 1.0 };

    if exp == 0 {
        if mant == 0 {
            0.0 * sign_mul
        } else {
            // Subnormal: 2^(-2) * (mant / 4)
            sign_mul * (mant as f64 / 4.0) * (2.0f64).powi(-2)
        }
    } else {
        // Normal: 2^(exp - 3) * (1 + mant / 4)
        sign_mul * (1.0 + mant as f64 / 4.0) * (2.0f64).powi(exp as i32 - 3)
    }
}

/// Encodes an f64 to OCP FP6 E3M2 (6-bit).
pub fn encode_fp6_e3m2(val: f64) -> u8 {
    let sign_bit = if val.is_sign_negative() { 1u8 << 5 } else { 0 };
    let abs_val = val.abs();
    if abs_val == 0.0 || val.is_nan() {
        return sign_bit;
    }
    // Max: exp=7, mant=3 -> 2^4 * 1.75 = 28.0
    if abs_val >= 28.0 {
        return sign_bit | 0x1F;
    }
    if abs_val < (2.0f64).powi(-2) {
        let mant = (abs_val / (2.0f64).powi(-4)).round() as u8;
        return sign_bit | (mant & 0x03);
    }
    let mut exp = (abs_val.log2().floor()) as i32;
    let mant_frac = abs_val / (2.0f64).powi(exp) - 1.0;
    let mut mant = (mant_frac * 4.0).round() as u8;
    if mant >= 4 {
        exp += 1;
        mant = 0;
    }
    let biased_exp = ((exp + 3).clamp(0, 7)) as u8;
    sign_bit | (biased_exp << 2) | (mant & 0x03)
}

/// Decodes an NVIDIA Blackwell / OCP FP4 E2M1 (4-bit) float to f64.
/// Layout: 1 sign, 2 exponent, 1 mantissa, bias 1.
pub fn decode_fp4_e2m1(bits: u8) -> f64 {
    let sign = (bits >> 3) & 1;
    let exp = (bits >> 1) & 0x03;
    let mant = bits & 0x01;

    let sign_mul = if sign == 1 { -1.0 } else { 1.0 };

    if exp == 0 {
        if mant == 0 {
            0.0 * sign_mul
        } else {
            // Subnormal: 2^(0) * (1 / 2) = 0.5
            sign_mul * 0.5
        }
    } else {
        // Normal: 2^(exp - 1) * (1 + mant * 0.5)
        sign_mul * (1.0 + mant as f64 * 0.5) * (2.0f64).powi(exp as i32 - 1)
    }
}

/// Encodes an f64 to NVIDIA Blackwell / OCP FP4 E2M1 (4-bit).
pub fn encode_fp4_e2m1(val: f64) -> u8 {
    let sign_bit = if val.is_sign_negative() { 1u8 << 3 } else { 0 };
    let abs_val = val.abs();
    if abs_val < 0.25 || val.is_nan() {
        return sign_bit;
    }
    // Supported positive levels: 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0
    let candidates = [
        (0.5, 0b0001u8),
        (1.0, 0b0010u8),
        (1.5, 0b0011u8),
        (2.0, 0b0100u8),
        (3.0, 0b0101u8),
        (4.0, 0b0110u8),
        (6.0, 0b0111u8),
    ];
    let mut best_code = 0b0111u8;
    let mut best_err = f64::MAX;
    for (v, code) in candidates {
        let err = (abs_val - v).abs();
        if err < best_err {
            best_err = err;
            best_code = code;
        }
    }
    sign_bit | best_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f16_roundtrip() {
        assert_eq!(decode_f16(0x3C00), 1.0);
        assert_eq!(decode_f16(0xBC00), -1.0);
        assert_eq!(decode_f16(0x4000), 2.0);
        assert_eq!(encode_f16(1.0), 0x3C00);
        assert_eq!(encode_f16(-1.0), 0xBC00);
        assert_eq!(encode_f16(2.0), 0x4000);
    }

    #[test]
    fn test_bf16_roundtrip() {
        assert_eq!(decode_bf16(0x3F80), 1.0);
        assert_eq!(decode_bf16(0xBF80), -1.0);
        assert_eq!(encode_bf16(1.0), 0x3F80);
        assert_eq!(encode_bf16(-1.0), 0xBF80);
    }

    #[test]
    fn test_fp8_e4m3() {
        assert_eq!(decode_fp8_e4m3(0x38), 1.0);
        assert_eq!(encode_fp8_e4m3(1.0), 0x38);
    }

    #[test]
    fn test_fp4_e2m1() {
        assert_eq!(decode_fp4_e2m1(0b0010), 1.0);
        assert_eq!(decode_fp4_e2m1(0b0001), 0.5);
        assert_eq!(decode_fp4_e2m1(0b0111), 6.0);
        assert_eq!(encode_fp4_e2m1(1.0), 0b0010);
        assert_eq!(encode_fp4_e2m1(0.5), 0b0001);
        assert_eq!(encode_fp4_e2m1(6.0), 0b0111);
    }
}
