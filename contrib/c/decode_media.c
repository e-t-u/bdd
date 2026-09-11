/**
 * decode_media.c - C demonstration of libbdd native C-ABI for multimedia parsing.
 *
 * Demonstrates:
 * - bdd_unpack_u64(): Parsing MP3 32-bit unaligned frame header
 * - bdd_unpack_u64(): Parsing MPEG-TS 188-byte packet header (13-bit PID)
 * - bdd_unpack_u64(): Parsing JPEG SOF0 4-bit chroma subsampling nibbles
 * - bdd_pack_u64()  : Repacking fields into integer units
 * - bdd_reverse_bits_u64(): Hardware-accelerated bit reversals
 */

#include "bdd.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>

static void demo_mp3(const char *filepath) {
    printf("=================================================================\n");
    printf(" 1. MP3 Frame Header Decoding via libbdd C-ABI\n");
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) {
        fprintf(stderr, "Failed to open %s\n", filepath);
        return;
    }
    uint8_t buf[4];
    if (fread(buf, 1, 4, f) != 4) {
        fprintf(stderr, "Failed to read 4 bytes from %s\n", filepath);
        fclose(f);
        return;
    }
    fclose(f);

    uint64_t unit = ((uint64_t)buf[0] << 24) |
                    ((uint64_t)buf[1] << 16) |
                    ((uint64_t)buf[2] << 8)  |
                    ((uint64_t)buf[3]);

    const char *pattern = "11U2U2U1U4U2U1U1U2U2U1U1U2U";
    uint64_t fields[16];
    int n = bdd_unpack_u64(pattern, unit, fields, 16);
    if (n < 0) {
        fprintf(stderr, "bdd_unpack_u64 failed with error code %d\n", n);
        return;
    }

    uint64_t sync     = fields[0];
    uint64_t ver      = fields[1];
    uint64_t layer    = fields[2];
    uint64_t prot     = fields[3];
    uint64_t br_idx   = fields[4];
    uint64_t freq_idx = fields[5];
    uint64_t pad      = fields[6];
    uint64_t mode     = fields[8];
    uint64_t orig     = fields[11];

    const char *ver_names[] = {"MPEG-2.5", "Reserved", "MPEG-2", "MPEG-1"};
    const char *layer_names[] = {"Reserved", "Layer III (MP3)", "Layer II", "Layer I"};
    int kbps_table[] = {0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320};
    const char *freq_names[] = {"44.1 kHz", "48.0 kHz", "32.0 kHz", "Reserved"};
    const char *mode_names[] = {"Stereo", "Joint Stereo", "Dual Channel", "Mono"};

    printf("  Input File:        %s\n", filepath);
    printf("  Raw Unit:          0x%08lX\n", (unsigned long)unit);
    printf("  Syncword:          0x%lX (%s)\n", (unsigned long)sync, sync == 0x7FF ? "Valid" : "Invalid");
    printf("  Format:            %s %s\n", ver_names[ver], layer_names[layer]);
    printf("  Bitrate:           %d kbps\n", kbps_table[br_idx]);
    printf("  Sampling Rate:     %s\n", freq_names[freq_idx]);
    printf("  Channel Mode:      %s\n", mode_names[mode]);
    printf("  Padding:           %s\n", pad ? "Yes" : "No");
    printf("  CRC:               %s\n", prot ? "None" : "16-bit CRC Present");
    printf("  Original/Copy:     %s\n", orig ? "Original" : "Copy");

    // Roundtrip verification: pack fields back into unit!
    uint64_t repacked = 0;
    int pack_res = bdd_pack_u64(pattern, fields, n, &repacked);
    printf("  Repack roundtrip:  0x%08lX (%s)\n",
           (unsigned long)repacked,
           (pack_res == 0 && repacked == unit) ? "BIT-EXACT MATCH" : "MISMATCH");
}

static void demo_mpeg_ts(const char *filepath) {
    printf("\n=================================================================\n");
    printf(" 2. MPEG Transport Stream Packet Header Parsing via libbdd\n");
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) return;

    uint8_t pkt[188];
    int pkt_count = 0;
    const char *pattern = "8U1U1U1U13U2U2U4U";
    uint64_t fields[8];

    while (fread(pkt, 1, 188, f) == 188) {
        uint64_t header = ((uint64_t)pkt[0] << 24) |
                          ((uint64_t)pkt[1] << 16) |
                          ((uint64_t)pkt[2] << 8)  |
                          ((uint64_t)pkt[3]);
        int n = bdd_unpack_u64(pattern, header, fields, 8);
        if (n == 8) {
            uint64_t sync    = fields[0];
            uint64_t pusi    = fields[2];
            uint64_t pid     = fields[4];
            uint64_t counter = fields[7];

            const char *desc = "Unknown";
            if (pid == 0) desc = "PAT";
            else if (pid == 256) desc = "Video (H.264)";
            else if (pid == 257) desc = "Audio (AAC)";

            printf("  Packet #%d: Sync=0x%02lX | PID=%4lu (0x%04lX) [%-13s] | PUSI=%lu | Counter=%lu\n",
                   pkt_count, (unsigned long)sync, (unsigned long)pid, (unsigned long)pid,
                   desc, (unsigned long)pusi, (unsigned long)counter);
        }
        pkt_count++;
    }
    fclose(f);
}

static void demo_jpeg(const char *filepath) {
    printf("\n=================================================================\n");
    printf(" 3. JPEG SOF0 Chroma Subsampling Nibbles via libbdd\n");
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) return;

    // Seek to component section: in sample.jpg, SOF0 is at byte 20, components at byte 30
    fseek(f, 30, SEEK_SET);
    uint8_t comp_buf[3];
    const char *comp_pattern = "8U4U4U8U";
    uint64_t fields[4];

    for (int i = 0; i < 3; i++) {
        if (fread(comp_buf, 1, 3, f) != 3) break;
        uint64_t comp_u24 = ((uint64_t)comp_buf[0] << 16) |
                            ((uint64_t)comp_buf[1] << 8)  |
                            ((uint64_t)comp_buf[2]);

        int n = bdd_unpack_u64(comp_pattern, comp_u24, fields, 4);
        if (n == 4) {
            uint64_t cid = fields[0];
            uint64_t h   = fields[1];
            uint64_t v   = fields[2];
            uint64_t q   = fields[3];
            const char *name = (i == 0) ? "Y (Luma)" : ((i == 1) ? "Cb (Chroma)" : "Cr (Chroma)");
            printf("  Component %lu [%-11s]: Subsampling=%lux%lu (%s) | QuantTable=%lu\n",
                   (unsigned long)cid, name, (unsigned long)h, (unsigned long)v,
                   (h == 2 && v == 2) ? "4:2:0 Subsampling" : "1x1", (unsigned long)q);
        }
    }
    fclose(f);
}

static void demo_bit_reversal(void) {
    printf("\n=================================================================\n");
    printf(" 4. Hardware Bit Reversal Acceleration (bdd_reverse_bits_u64)\n");
    printf("=================================================================\n");
    uint64_t val = 0x123456789ABCDEF0ULL;
    uint64_t rev64 = bdd_reverse_bits_u64(val, 64);
    uint64_t rev8  = bdd_reverse_bits_u64(0x0F, 8);

    printf("  Original 64-bit: 0x%016lX\n", (unsigned long)val);
    printf("  Reversed 64-bit: 0x%016lX\n", (unsigned long)rev64);
    printf("  Reversed 8-bit : 0x0F -> 0x%02lX (Expected 0xF0)\n", (unsigned long)rev8);
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
    const char *mp3_file = (argc > 1) ? argv[1] : find_file("data/sample.mp3");
    const char *ts_file  = (argc > 2) ? argv[2] : find_file("data/sample.ts");
    const char *jpg_file = (argc > 3) ? argv[3] : find_file("data/sample.jpg");

    demo_mp3(mp3_file);
    demo_mpeg_ts(ts_file);
    demo_jpeg(jpg_file);
    demo_bit_reversal();

    printf("\nAll C multimedia examples completed successfully!\n");
    return 0;
}
