/**
 * decode_ai_weights.c - C demonstration of libbdd native AI floating point codecs.
 *
 * Demonstrates:
 * - bdd_decode_fp4_e2m1()  : NVIDIA Blackwell NVFP4
 * - bdd_decode_fp6_e3m2()  : OCP Microscaling FP6
 * - bdd_decode_fp8_e4m3()  : OCP / NVIDIA Hopper FP8 (E4M3)
 * - bdd_decode_fp8_e5m2()  : OCP / NVIDIA Ada FP8 (E5M2)
 * - bdd_decode_bf16()      : Google Brain Bfloat16
 * - bdd_decode_f16()       : IEEE 754 Half-Precision Float
 * - bdd_unpack_u64()       : Unpacking sub-byte and unaligned float bitfields
 */

#include "bdd.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <math.h>

static void demo_float_conversions(void) {
    printf("=================================================================\n");
    printf(" 1. Direct AI Float Codecs (Roundtrip Tests)\n");
    printf("=================================================================\n");

    // 1. NVIDIA Blackwell NVFP4 (E2M1)
    double fp4_val = 1.5;
    uint8_t fp4_bits = bdd_encode_fp4_e2m1(fp4_val);
    double fp4_dec = bdd_decode_fp4_e2m1(fp4_bits);
    printf("  NVFP4 (E2M1): %.2f -> 0x%02X -> %.2f (%s)\n",
           fp4_val, fp4_bits, fp4_dec, (fabs(fp4_dec - fp4_val) < 1e-4) ? "OK" : "FAIL");

    // 2. OCP FP6 (E3M2)
    double fp6_val = 2.0;
    uint8_t fp6_bits = bdd_encode_fp6_e3m2(fp6_val);
    double fp6_dec = bdd_decode_fp6_e3m2(fp6_bits);
    printf("  FP6   (E3M2): %.2f -> 0x%02X -> %.2f (%s)\n",
           fp6_val, fp6_bits, fp6_dec, (fabs(fp6_dec - fp6_val) < 1e-4) ? "OK" : "FAIL");

    // 3. OCP FP8 (E4M3FN - Hopper H100)
    double fp8_val = 3.5;
    uint8_t fp8_bits = bdd_encode_fp8_e4m3(fp8_val);
    double fp8_dec = bdd_decode_fp8_e4m3(fp8_bits);
    printf("  FP8 (E4M3FN): %.2f -> 0x%02X -> %.2f (%s)\n",
           fp8_val, fp8_bits, fp8_dec, (fabs(fp8_dec - fp8_val) < 1e-4) ? "OK" : "FAIL");

    // 4. OCP FP8 (E5M2 - Ada Lovelace)
    double fp8e5_val = -12.0;
    uint8_t fp8e5_bits = bdd_encode_fp8_e5m2(fp8e5_val);
    double fp8e5_dec = bdd_decode_fp8_e5m2(fp8e5_bits);
    printf("  FP8   (E5M2): %.2f -> 0x%02X -> %.2f (%s)\n",
           fp8e5_val, fp8e5_bits, fp8e5_dec, (fabs(fp8e5_dec - fp8e5_val) < 1e-4) ? "OK" : "FAIL");

    // 5. Google Brain Bfloat16 (BF16)
    double bf16_val = 42.0;
    uint16_t bf16_bits = bdd_encode_bf16(bf16_val);
    double bf16_dec = bdd_decode_bf16(bf16_bits);
    printf("  Bfloat16    : %.2f -> 0x%04X -> %.2f (%s)\n",
           bf16_val, bf16_bits, bf16_dec, (fabs(bf16_dec - bf16_val) < 1e-4) ? "OK" : "FAIL");

    // 6. IEEE 754 Half-Precision (FP16)
    double fp16_val = -0.125;
    uint16_t fp16_bits = bdd_encode_f16(fp16_val);
    double fp16_dec = bdd_decode_f16(fp16_bits);
    printf("  FP16        : %.3f -> 0x%04X -> %.3f (%s)\n",
           fp16_val, fp16_bits, fp16_dec, (fabs(fp16_dec - fp16_val) < 1e-4) ? "OK" : "FAIL");
}

static void demo_nvfp4_tensor(const char *filepath) {
    printf("\n=================================================================\n");
    printf(" 2. Decoding NVIDIA Blackwell NVFP4 Weights (%s)\n", filepath);
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) return;

    uint8_t buf[8];
    size_t bytes_read = fread(buf, 1, 8, f);
    fclose(f);

    uint64_t fields[2];
    for (size_t i = 0; i < bytes_read && i < 4; i++) {
        bdd_unpack_u64("4U4U", buf[i], fields, 2);
        double w0 = bdd_decode_fp4_e2m1((uint8_t)fields[0]);
        double w1 = bdd_decode_fp4_e2m1((uint8_t)fields[1]);
        printf("  Byte #%zu (0x%02X) -> [w0 (bits: 0x%lX) = %5.2f, w1 (bits: 0x%lX) = %5.2f]\n",
               i, buf[i], (unsigned long)fields[0], w0, (unsigned long)fields[1], w1);
    }
}

static void demo_fp6_tensor(const char *filepath) {
    printf("\n=================================================================\n");
    printf(" 3. Decoding OCP Microscaling FP6 Weights (%s)\n", filepath);
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) return;

    uint8_t buf[3];
    uint64_t fields[4];
    int chunk = 0;

    while (fread(buf, 1, 3, f) == 3) {
        uint64_t u24 = ((uint64_t)buf[0] << 16) |
                       ((uint64_t)buf[1] << 8)  |
                       ((uint64_t)buf[2]);
        bdd_unpack_u64("6U6U6U6U", u24, fields, 4);

        printf("  Chunk #%d (24-bit 0x%06lX) -> FP6 Weights: [", chunk, (unsigned long)u24);
        for (int i = 0; i < 4; i++) {
            double val = bdd_decode_fp6_e3m2((uint8_t)fields[i]);
            printf("%.2f%s", val, (i < 3) ? ", " : "]\n");
        }
        chunk++;
    }
    fclose(f);
}

static void demo_safetensors(const char *filepath) {
    printf("\n=================================================================\n");
    printf(" 4. Inspecting Safetensors Model Weights (%s)\n", filepath);
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) return;

    uint64_t header_size = 0;
    if (fread(&header_size, 1, 8, f) != 8) {
        fclose(f);
        return;
    }
    printf("  Header JSON Size: %lu bytes\n", (unsigned long)header_size);

    // Seek directly to tensors buffer
    fseek(f, 8 + header_size, SEEK_SET);

    // Read 4 FP8 E4M3 weights
    uint8_t fp8_buf[4];
    if (fread(fp8_buf, 1, 4, f) == 4) {
        printf("  Decoded FP8 (E4M3) Tensor: [");
        for (int i = 0; i < 4; i++) {
            double v = bdd_decode_fp8_e4m3(fp8_buf[i]);
            printf("%.2f%s", v, (i < 3) ? ", " : "]\n");
        }
    }

    // Read 2 BF16 weights
    uint16_t bf16_buf[2];
    if (fread(bf16_buf, 2, 2, f) == 2) {
        printf("  Decoded BF16 Tensor      : [");
        for (int i = 0; i < 2; i++) {
            double v = bdd_decode_bf16(bf16_buf[i]);
            printf("%.2f%s", v, (i < 1) ? ", " : "]\n");
        }
    }

    // Read 2 FP16 weights
    uint16_t fp16_buf[2];
    if (fread(fp16_buf, 2, 2, f) == 2) {
        printf("  Decoded FP16 Tensor      : [");
        for (int i = 0; i < 2; i++) {
            double v = bdd_decode_f16(fp16_buf[i]);
            printf("%.2f%s", v, (i < 1) ? ", " : "]\n");
        }
    }

    fclose(f);
}

static const char *find_file(const char *rel_path) {
    static char buf[512];
    FILE *f = fopen(rel_path, "rb");
    if (f) { fclose(f); return rel_path; }
    snprintf(buf, sizeof(buf), "contrib/%s", rel_path);
    f = fopen(buf, "rb");
    if (f) { fclose(f); return buf; }
    snprintf(buf, sizeof(buf), "../%s", rel_path);
    f = fopen(buf, "rb");
    if (f) { fclose(f); return buf; }
    return rel_path;
}

int main(int argc, char **argv) {
    const char *nvfp4_file       = (argc > 1) ? argv[1] : find_file("data/sample_nvfp4.bin");
    const char *fp6_file         = (argc > 2) ? argv[2] : find_file("data/sample_fp6.bin");
    const char *safetensors_file = (argc > 3) ? argv[3] : find_file("data/sample.safetensors");

    demo_float_conversions();
    demo_nvfp4_tensor(nvfp4_file);
    demo_fp6_tensor(fp6_file);
    demo_safetensors(safetensors_file);

    printf("\nAll C AI weight examples completed successfully!\n");
    return 0;
}
