#!/usr/bin/env python3
"""
decode_network.py: Python demonstration of libbdd C-ABI bindings for network protocol analysis.
Demonstrates:
- RFC 791 IPv4 20-byte base header parsing via 32-bit word patterns (4U4U6U2U16U, 16U3U13U, 8U8U16U)
- RFC 768 UDP 8-byte datagram header parsing via single 64-bit unit (16U16U16U16U)
- RFC 793 TCP 20-byte base segment parsing with sub-byte control flag extraction (4U3U1B1B1B1B1B1B1B1B1B16U)
- Bidirectional roundtrip packing with bdd.pack()
- Multi-packet capture stream dissection
"""

import os
import sys
import struct
import socket

# Ensure python/ directory is in sys.path
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
BASE_DIR = os.path.dirname(os.path.dirname(SCRIPT_DIR))
DATA_DIR = os.path.join(os.path.dirname(SCRIPT_DIR), "data")
sys.path.insert(0, os.path.join(BASE_DIR, "python"))

from bdd import Bdd

def format_ip(ip_int: int) -> str:
    """Format 32-bit integer as dotted-decimal IPv4 address."""
    return socket.inet_ntoa(struct.pack(">I", ip_int))

def decode_ipv4_header(bdd: Bdd, data: bytes):
    """Decode 20-byte IPv4 base header using libbdd patterns on 32-bit words."""
    if len(data) < 20:
        raise ValueError("IPv4 header requires at least 20 bytes")

    w0, w1, w2, w3, w4 = struct.unpack(">IIIII", data[:20])

    version, ihl, dscp, ecn, total_length = bdd.unpack("4U4U6U2U16U", w0)
    pkt_id, flags, frag_offset = bdd.unpack("16U3U13U", w1)
    ttl, protocol, checksum = bdd.unpack("8U8U16U", w2)
    src_ip = w3
    dst_ip = w4

    proto_name = {1: "ICMP", 6: "TCP", 17: "UDP"}.get(protocol, f"Unknown ({protocol})")
    df_flag = bool(flags & 0x2)
    mf_flag = bool(flags & 0x1)

    return {
        "version": version,
        "ihl": ihl,
        "header_len": ihl * 4,
        "dscp": dscp,
        "ecn": ecn,
        "total_length": total_length,
        "id": pkt_id,
        "flags": flags,
        "df": df_flag,
        "mf": mf_flag,
        "frag_offset": frag_offset,
        "ttl": ttl,
        "protocol": protocol,
        "proto_name": proto_name,
        "checksum": checksum,
        "src_ip": format_ip(src_ip),
        "dst_ip": format_ip(dst_ip),
    }

def decode_udp_header(bdd: Bdd, data: bytes):
    """Decode 8-byte UDP datagram header using a single 64-bit unit."""
    if len(data) < 8:
        raise ValueError("UDP header requires at least 8 bytes")

    u64 = struct.unpack(">Q", data[:8])[0]
    pattern = "16U16U16U16U"
    fields = bdd.unpack(pattern, u64)
    src_port, dst_port, length, checksum = fields

    # Verify roundtrip packing
    repacked = bdd.pack(pattern, fields)
    assert repacked == u64, "UDP roundtrip packing mismatch!"

    return {
        "src_port": src_port,
        "dst_port": dst_port,
        "length": length,
        "checksum": checksum,
    }

def decode_tcp_header(bdd: Bdd, data: bytes):
    """Decode 20-byte TCP base segment header with sub-byte flag extraction."""
    if len(data) < 20:
        raise ValueError("TCP header requires at least 20 bytes")

    w0, w1, w2, w3, w4 = struct.unpack(">IIIII", data[:20])

    src_port, dst_port = bdd.unpack("16U16U", w0)
    seq_num = w1
    ack_num = w2

    # Word 3: data_offset(4), reserved(3), 9 flags (1 bit each), window(16)
    offset, res, ns, cwr, ece, urg, ack, psh, rst, syn, fin, win = bdd.unpack(
        "4U3U1B1B1B1B1B1B1B1B1B16U", w3
    )
    checksum, urg_ptr = bdd.unpack("16U16U", w4)

    active_flags = []
    if cwr: active_flags.append("CWR")
    if ece: active_flags.append("ECE")
    if urg: active_flags.append("URG")
    if ack: active_flags.append("ACK")
    if psh: active_flags.append("PSH")
    if rst: active_flags.append("RST")
    if syn: active_flags.append("SYN")
    if fin: active_flags.append("FIN")

    return {
        "src_port": src_port,
        "dst_port": dst_port,
        "seq_num": seq_num,
        "ack_num": ack_num,
        "data_offset": offset,
        "header_len": offset * 4,
        "active_flags": active_flags,
        "syn": bool(syn),
        "ack": bool(ack),
        "psh": bool(psh),
        "fin": bool(fin),
        "window_size": win,
        "checksum": checksum,
        "urg_ptr": urg_ptr,
    }

def demo_standalone(bdd: Bdd):
    print("=================================================================")
    print(" 1. Standalone Network Header Decoding via libbdd")
    print("=================================================================")

    # 1.1 IPv4 Header
    ipv4_path = os.path.join(DATA_DIR, "sample_ipv4.bin")
    with open(ipv4_path, "rb") as f:
        ip_info = decode_ipv4_header(bdd, f.read())
    print("  --- IPv4 Header ---")
    print(f"    Version / IHL:    IPv{ip_info['version']}, {ip_info['header_len']} bytes")
    print(f"    Total Length:     {ip_info['total_length']} bytes")
    print(f"    Identification:   0x{ip_info['id']:04X} ({ip_info['id']})")
    print(f"    Flags:            DF={'Yes' if ip_info['df'] else 'No'}, MF={'Yes' if ip_info['mf'] else 'No'}")
    print(f"    TTL / Protocol:   {ip_info['ttl']} hops, {ip_info['proto_name']}")
    print(f"    Checksum:         0x{ip_info['checksum']:04X}")
    print(f"    Routing:          {ip_info['src_ip']} -> {ip_info['dst_ip']}")

    # 1.2 UDP Header
    udp_path = os.path.join(DATA_DIR, "sample_udp.bin")
    with open(udp_path, "rb") as f:
        udp_info = decode_udp_header(bdd, f.read())
    print("\n  --- UDP Header ---")
    print(f"    Source Port:      {udp_info['src_port']}")
    print(f"    Destination Port: {udp_info['dst_port']}")
    print(f"    Length:           {udp_info['length']} bytes")
    print(f"    Checksum:         0x{udp_info['checksum']:04X}")

    # 1.3 TCP Header
    tcp_path = os.path.join(DATA_DIR, "sample_tcp.bin")
    with open(tcp_path, "rb") as f:
        tcp_info = decode_tcp_header(bdd, f.read())
    print("\n  --- TCP Header ---")
    print(f"    Source Port:      {tcp_info['src_port']}")
    print(f"    Destination Port: {tcp_info['dst_port']}")
    print(f"    Seq / Ack:        {tcp_info['seq_num']} / {tcp_info['ack_num']}")
    print(f"    Header Length:    {tcp_info['header_len']} bytes")
    print(f"    Active Flags:     [{', '.join(tcp_info['active_flags'])}]")
    print(f"    Window Size:      {tcp_info['window_size']}")
    print(f"    Checksum:         0x{tcp_info['checksum']:04X}")

def demo_packet_stream(bdd: Bdd):
    print("\n=================================================================")
    print(" 2. Multi-Packet Capture Stream Dissection")
    print("=================================================================")
    packets_path = os.path.join(DATA_DIR, "sample_packets.bin")

    with open(packets_path, "rb") as f:
        stream = f.read()

    offset = 0
    pkt_num = 1

    while offset < len(stream):
        ip_info = decode_ipv4_header(bdd, stream[offset:])
        ip_len = ip_info["total_length"]
        pkt_bytes = stream[offset : offset + ip_len]
        header_len = ip_info["header_len"]
        payload_offset = header_len

        print(f"\n  Packet #{pkt_num} [{ip_len} bytes total]:")
        print(f"    IP Layer:     {ip_info['src_ip']} -> {ip_info['dst_ip']} ({ip_info['proto_name']})")

        if ip_info["protocol"] == 17:  # UDP
            udp_info = decode_udp_header(bdd, pkt_bytes[payload_offset:])
            print(f"    UDP Layer:    Port {udp_info['src_port']} -> {udp_info['dst_port']} (Length: {udp_info['length']}B)")
            dns_bytes = pkt_bytes[payload_offset + 8 :]
            if len(dns_bytes) >= 4:
                dns_u32 = struct.unpack(">I", dns_bytes[:4])[0]
                tx_id, flags = bdd.unpack("16U16U", dns_u32)
                print(f"    DNS Payload:  Query ID=0x{tx_id:04X}, Flags=0x{flags:04X}")

        elif ip_info["protocol"] == 6:  # TCP
            tcp_info = decode_tcp_header(bdd, pkt_bytes[payload_offset:])
            flags_str = ", ".join(tcp_info["active_flags"])
            print(f"    TCP Layer:    Port {tcp_info['src_port']} -> {tcp_info['dst_port']} [Flags: {flags_str}] Seq={tcp_info['seq_num']}")
            tcp_hdr_len = tcp_info["header_len"]
            app_payload = pkt_bytes[payload_offset + tcp_hdr_len :]
            if app_payload:
                preview = repr(app_payload.decode("latin1"))
                print(f"    App Payload:  {preview}")

        offset += ip_len
        pkt_num += 1

def main():
    bdd = Bdd()
    demo_standalone(bdd)
    demo_packet_stream(bdd)
    print("\nAll Python network protocol examples executed successfully!")

if __name__ == "__main__":
    main()
