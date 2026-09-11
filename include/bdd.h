#ifndef BDD_H
#define BDD_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Reverses the lowest `bits` bits of a 64-bit integer.
 */
uint64_t bdd_reverse_bits_u64(uint64_t val, size_t bits);

/**
 * Decodes an IEEE 754 half-precision float to double.
 */
double bdd_decode_f16(uint16_t bits);

/**
 * Encodes a double to IEEE 754 half-precision float.
 */
uint16_t bdd_encode_f16(double val);

/**
 * Decodes a Bfloat16 float to double.
 */
double bdd_decode_bf16(uint16_t bits);

/**
 * Encodes a double to Bfloat16.
 */
uint16_t bdd_encode_bf16(double val);

/**
 * Decodes an OCP FP8 E4M3FN float to double.
 */
double bdd_decode_fp8_e4m3(uint8_t bits);

/**
 * Encodes a double to OCP FP8 E4M3FN.
 */
uint8_t bdd_encode_fp8_e4m3(double val);

/**
 * Decodes an NVIDIA Blackwell / OCP FP4 E2M1 float to double.
 */
double bdd_decode_fp4_e2m1(uint8_t bits);

/**
 * Encodes a double to NVIDIA Blackwell / OCP FP4 E2M1.
 */
uint8_t bdd_encode_fp4_e2m1(double val);

/**
 * Unpacks a 64-bit unit according to a pattern string (e.g. "4U4U") into out_fields.
 * Returns number of fields written, or negative error code.
 */
int bdd_unpack_u64(const char *pattern, uint64_t unit, uint64_t *out_fields, size_t max_fields);

/**
 * Packs an array of u64 field values into a 64-bit unit according to a pattern string.
 * Returns 0 on success, negative error code on failure.
 */
int bdd_pack_u64(const char *pattern, const uint64_t *fields, size_t num_fields, uint64_t *out_unit);

#ifdef __cplusplus
}
#endif

#endif // BDD_H
