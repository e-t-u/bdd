/**
 * decode_network.c - C demonstration of libbdd native C-ABI for network packet analysis.
 *
 * Demonstrates:
 * - bdd_unpack_u64(): Parsing RFC 791 IPv4 20-byte base header (32-bit word slicing)
 * - bdd_unpack_u64(): Parsing RFC 768 UDP 8-byte datagram header (single 64-bit unit)
 * - bdd_unpack_u64(): Parsing RFC 793 TCP 20-byte base header with discrete 1-bit flags
 * - bdd_pack_u64()  : Bit-exact roundtrip repacking
 * - Dissecting raw multi-packet captures (sample_packets.bin)
 */

#include "bdd.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <arpa/inet.h>

static uint32_t read_u32_be(const uint8_t *p) {
    return ((uint32_t)p[0] << 24) |
           ((uint32_t)p[1] << 16) |
           ((uint32_t)p[2] << 8)  |
           ((uint32_t)p[3]);
}

static uint64_t read_u64_be(const uint8_t *p) {
    uint64_t hi = read_u32_be(p);
    uint64_t lo = read_u32_be(p + 4);
    return (hi << 32) | lo;
}

static void ip_to_str(uint32_t ip, char *buf, size_t maxlen) {
    uint32_t net_ip = htonl(ip);
    inet_ntop(AF_INET, &net_ip, buf, maxlen);
}

static void demo_ipv4(const char *filepath) {
    printf("=================================================================\n");
    printf(" 1. IPv4 Header Parsing via libbdd C-ABI\n");
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) {
        fprintf(stderr, "Failed to open %s\n", filepath);
        return;
    }
    uint8_t hdr[20];
    if (fread(hdr, 1, 20, f) != 20) {
        fprintf(stderr, "Failed to read 20 bytes from %s\n", filepath);
        fclose(f);
        return;
    }
    fclose(f);

    uint32_t w0 = read_u32_be(hdr);
    uint32_t w1 = read_u32_be(hdr + 4);
    uint32_t w2 = read_u32_be(hdr + 8);
    uint32_t src_ip = read_u32_be(hdr + 12);
    uint32_t dst_ip = read_u32_be(hdr + 16);

    uint64_t f0[5], f1[3], f2[3];
    bdd_unpack_u64("4U4U6U2U16U", w0, f0, 5);
    bdd_unpack_u64("16U3U13U", w1, f1, 3);
    bdd_unpack_u64("8U8U16U", w2, f2, 3);

    char src_str[INET_ADDRSTRLEN], dst_str[INET_ADDRSTRLEN];
    ip_to_str(src_ip, src_str, sizeof(src_str));
    ip_to_str(dst_ip, dst_str, sizeof(dst_str));

    const char *proto_name = (f2[1] == 17) ? "UDP" : ((f2[1] == 6) ? "TCP" : "Other");

    printf("  Input File:        %s\n", filepath);
    printf("  Version / IHL:     IPv%lu, %lu bytes (IHL: %lu)\n",
           (unsigned long)f0[0], (unsigned long)(f0[1] * 4), (unsigned long)f0[1]);
    printf("  Total Length:      %lu bytes\n", (unsigned long)f0[4]);
    printf("  Packet ID:         0x%04lX (%lu)\n", (unsigned long)f1[0], (unsigned long)f1[0]);
    printf("  Flags / Frag Off:  0x%lX [DF: %s] / %lu\n",
           (unsigned long)f1[1], (f1[1] & 0x2) ? "Yes" : "No", (unsigned long)f1[2]);
    printf("  TTL / Protocol:    %lu hops, %s (%lu)\n",
           (unsigned long)f2[0], proto_name, (unsigned long)f2[1]);
    printf("  Header Checksum:   0x%04lX\n", (unsigned long)f2[2]);
    printf("  Source IP:         %s\n", src_str);
    printf("  Destination IP:    %s\n", dst_str);

    // Roundtrip verification
    uint64_t repacked_w0 = 0;
    bdd_pack_u64("4U4U6U2U16U", f0, 5, &repacked_w0);
    printf("  Roundtrip w0 pack: 0x%08lX (%s)\n",
           (unsigned long)repacked_w0,
           (repacked_w0 == w0) ? "BIT-EXACT MATCH" : "MISMATCH");
}

static void demo_udp(const char *filepath) {
    printf("\n=================================================================\n");
    printf(" 2. UDP Datagram Header Parsing via libbdd C-ABI\n");
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) {
        fprintf(stderr, "Failed to open %s\n", filepath);
        return;
    }
    uint8_t hdr[8];
    if (fread(hdr, 1, 8, f) != 8) {
        fclose(f);
        return;
    }
    fclose(f);

    uint64_t u64 = read_u64_be(hdr);
    uint64_t fields[4];
    bdd_unpack_u64("16U16U16U16U", u64, fields, 4);

    printf("  Input File:        %s\n", filepath);
    printf("  Source Port:       %lu\n", (unsigned long)fields[0]);
    printf("  Destination Port:  %lu%s\n",
           (unsigned long)fields[1], (fields[1] == 53) ? " (DNS)" : "");
    printf("  Datagram Length:   %lu bytes\n", (unsigned long)fields[2]);
    printf("  Checksum:          0x%04lX\n", (unsigned long)fields[3]);

    uint64_t repacked = 0;
    bdd_pack_u64("16U16U16U16U", fields, 4, &repacked);
    printf("  Roundtrip pack:    0x%016lX (%s)\n",
           (unsigned long)repacked,
           (repacked == u64) ? "BIT-EXACT MATCH" : "MISMATCH");
}

static void demo_tcp(const char *filepath) {
    printf("\n=================================================================\n");
    printf(" 3. TCP Base Segment Header with Discrete Sub-Byte Flags\n");
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) {
        fprintf(stderr, "Failed to open %s\n", filepath);
        return;
    }
    uint8_t hdr[20];
    if (fread(hdr, 1, 20, f) != 20) {
        fclose(f);
        return;
    }
    fclose(f);

    uint32_t w0 = read_u32_be(hdr);
    uint32_t seq = read_u32_be(hdr + 4);
    uint32_t ack = read_u32_be(hdr + 8);
    uint32_t w3 = read_u32_be(hdr + 12);
    uint32_t w4 = read_u32_be(hdr + 16);

    uint64_t ports[2], flags_and_win[12], csum_urg[2];
    bdd_unpack_u64("16U16U", w0, ports, 2);
    bdd_unpack_u64("4U3U1B1B1B1B1B1B1B1B1B16U", w3, flags_and_win, 12);
    bdd_unpack_u64("16U16U", w4, csum_urg, 2);

    uint64_t offset = flags_and_win[0];
    uint64_t syn    = flags_and_win[9];
    uint64_t ack_f  = flags_and_win[6];
    uint64_t psh_f  = flags_and_win[7];
    uint64_t fin_f  = flags_and_win[10];
    uint64_t win    = flags_and_win[11];

    printf("  Input File:        %s\n", filepath);
    printf("  Source Port:       %lu\n", (unsigned long)ports[0]);
    printf("  Destination Port:  %lu%s\n",
           (unsigned long)ports[1], (ports[1] == 443) ? " (HTTPS)" : ((ports[1] == 80) ? " (HTTP)" : ""));
    printf("  Sequence Number:   %u\n", seq);
    printf("  Ack Number:        %u\n", ack);
    printf("  Header Length:     %lu bytes (Data Offset: %lu)\n",
           (unsigned long)(offset * 4), (unsigned long)offset);
    printf("  TCP Flags:         SYN=%lu ACK=%lu PSH=%lu FIN=%lu\n",
           (unsigned long)syn, (unsigned long)ack_f, (unsigned long)psh_f, (unsigned long)fin_f);
    printf("  Window Size:       %lu\n", (unsigned long)win);
    printf("  Checksum:          0x%04lX\n", (unsigned long)csum_urg[0]);
}

static void demo_stream(const char *filepath) {
    printf("\n=================================================================\n");
    printf(" 4. Multi-Packet Stream Capture Dissection\n");
    printf("=================================================================\n");
    FILE *f = fopen(filepath, "rb");
    if (!f) {
        fprintf(stderr, "Failed to open %s\n", filepath);
        return;
    }

    uint8_t pkt[256];
    int pkt_idx = 1;

    while (fread(pkt, 1, 20, f) == 20) {
        uint32_t w0 = read_u32_be(pkt);
        uint32_t w2 = read_u32_be(pkt + 8);
        uint32_t src_ip = read_u32_be(pkt + 12);
        uint32_t dst_ip = read_u32_be(pkt + 16);

        uint64_t f0[5], f2[3];
        bdd_unpack_u64("4U4U6U2U16U", w0, f0, 5);
        bdd_unpack_u64("8U8U16U", w2, f2, 3);

        uint64_t total_len = f0[4];
        uint64_t ihl_bytes = f0[1] * 4;
        uint64_t proto     = f2[1];

        char src_str[INET_ADDRSTRLEN], dst_str[INET_ADDRSTRLEN];
        ip_to_str(src_ip, src_str, sizeof(src_str));
        ip_to_str(dst_ip, dst_str, sizeof(dst_str));

        // Read remaining packet bytes
        size_t rem = (size_t)total_len - 20;
        if (fread(pkt + 20, 1, rem, f) != rem) break;

        const uint8_t *payload = pkt + ihl_bytes;

        printf("  Packet #%d [%lu bytes]: %s -> %s [Proto: %s]\n",
               pkt_idx++, (unsigned long)total_len, src_str, dst_str,
               (proto == 17) ? "UDP" : ((proto == 6) ? "TCP" : "Other"));

        if (proto == 17) {
            uint64_t udp_unit = read_u64_be(payload);
            uint64_t udp_fields[4];
            bdd_unpack_u64("16U16U16U16U", udp_unit, udp_fields, 4);
            printf("    UDP: Port %lu -> %lu (Len %luB)\n",
                   (unsigned long)udp_fields[0], (unsigned long)udp_fields[1], (unsigned long)udp_fields[2]);
        } else if (proto == 6) {
            uint32_t tcp_w0 = read_u32_be(payload);
            uint32_t tcp_seq = read_u32_be(payload + 4);
            uint32_t tcp_w3 = read_u32_be(payload + 12);
            uint64_t ports[2], f3[12];
            bdd_unpack_u64("16U16U", tcp_w0, ports, 2);
            bdd_unpack_u64("4U3U1B1B1B1B1B1B1B1B1B16U", tcp_w3, f3, 12);

            printf("    TCP: Port %lu -> %lu | Seq=%u | SYN=%lu ACK=%lu PSH=%lu\n",
                   (unsigned long)ports[0], (unsigned long)ports[1], tcp_seq,
                   (unsigned long)f3[9], (unsigned long)f3[6], (unsigned long)f3[7]);
        }
    }
    fclose(f);
}

static char *resolve_path(const char *rel_path) {
    char *buf = malloc(512);
    if (!buf) return NULL;

    FILE *f = fopen(rel_path, "rb");
    if (f) { fclose(f); strcpy(buf, rel_path); return buf; }

    snprintf(buf, 512, "contrib/%s", rel_path);
    f = fopen(buf, "rb");
    if (f) { fclose(f); return buf; }

    snprintf(buf, 512, "../%s", rel_path);
    f = fopen(buf, "rb");
    if (f) { fclose(f); return buf; }

    strcpy(buf, rel_path);
    return buf;
}

int main(int argc, char **argv) {
    char *ipv4_file    = (argc > 1) ? strdup(argv[1]) : resolve_path("data/sample_ipv4.bin");
    char *udp_file     = (argc > 2) ? strdup(argv[2]) : resolve_path("data/sample_udp.bin");
    char *tcp_file     = (argc > 3) ? strdup(argv[3]) : resolve_path("data/sample_tcp.bin");
    char *packets_file = (argc > 4) ? strdup(argv[4]) : resolve_path("data/sample_packets.bin");

    demo_ipv4(ipv4_file);
    demo_udp(udp_file);
    demo_tcp(tcp_file);
    demo_stream(packets_file);

    free(ipv4_file);
    free(udp_file);
    free(tcp_file);
    free(packets_file);

    printf("\nAll C network protocol examples completed successfully!\n");
    return 0;
}
