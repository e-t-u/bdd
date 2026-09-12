#!/usr/bin/env bash
# decode_network.sh: Inspect RFC-compliant IPv4, UDP, and TCP headers using bdd
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BDD="${BASE_DIR}/target/release/bdd"
DATA_DIR="${SCRIPT_DIR}/../data"

if [ ! -x "${BDD}" ]; then
    BDD="bdd"
fi

IPV4_SAMPLE="${DATA_DIR}/sample_ipv4.bin"
UDP_SAMPLE="${DATA_DIR}/sample_udp.bin"
TCP_SAMPLE="${DATA_DIR}/sample_tcp.bin"
PACKETS_SAMPLE="${DATA_DIR}/sample_packets.bin"

if [ ! -f "${IPV4_SAMPLE}" ] || [ ! -f "${PACKETS_SAMPLE}" ]; then
    echo "Sample data files missing. Run 'make samples' or python3 contrib/python/generate_samples.py first."
    exit 1
fi

echo "================================================================="
echo " Network Protocol Header Dissection with bdd"
echo " Bit-exact parsing of IPv4 (160b), UDP (64b), and TCP (160b)"
echo "================================================================="

# JQ helper for formatting 32-bit big-endian IPv4 integer to dotted-quad
JQ_IP_HELPER='
def to_ip: [
    (. / 16777216 | floor % 256),
    (. / 65536 | floor % 256),
    (. / 256 | floor % 256),
    (. % 256)
] | map(tostring) | join(".");
'

echo -e "\n--- Step 1: Standalone IPv4 Header Decoding (${IPV4_SAMPLE}) ---"
"${BDD}" --input-file="${IPV4_SAMPLE}" \
        --preset=ipv4-header \
        --output-json \
        --json-object | \
jq -r "${JQ_IP_HELPER}"'
  (if .protocol == 17 then "UDP (17)"
   elif .protocol == 6 then "TCP (6)"
   elif .protocol == 1 then "ICMP (1)"
   else "Unknown (\(.protocol))" end) as $proto |
  (if (.flags % 4 >= 2) then "DF (Do Not Fragment)" else "None" end) as $df |
  "  IPv4 Header:
    Version:        \(.version)
    Header Length:  \(.ihl * 4) bytes (IHL: \(.ihl))
    DSCP / ECN:     \(.dscp) / \(.ecn)
    Total Length:   \(.total_length) bytes
    Packet ID:      0x\(.id | tostring) (\(.id))
    Flags:          0x\(.flags | tostring) [\($df)]
    Frag Offset:    \(.frag_offset)
    Time to Live:   \(.ttl) hops
    Protocol:       \($proto)
    Header Checksum:0x\(.checksum | tostring)
    Source IP:      \(.src_ip | to_ip)
    Destination IP: \(.dst_ip | to_ip)"
'

echo -e "\n--- Step 2: Standalone UDP Header Decoding (${UDP_SAMPLE}) ---"
"${BDD}" --input-file="${UDP_SAMPLE}" \
        --preset=udp-header \
        --output-json \
        --json-object | \
jq -r '
  "  UDP Datagram Header:
    Source Port:      \(.src_port)
    Destination Port: \(.dst_port) (\(if .dst_port == 53 then "DNS" else "Other" end))
    Length:           \(.length) bytes
    Checksum:         0x\(.checksum | tostring)"
'

echo -e "\n--- Step 3: Standalone TCP Header Decoding (${TCP_SAMPLE}) ---"
"${BDD}" --input-file="${TCP_SAMPLE}" \
        --preset=tcp-header \
        --output-json \
        --json-object | \
jq -r '
  . as $root |
  [
    (if .cwr == 1 then "CWR" else empty end),
    (if .ece == 1 then "ECE" else empty end),
    (if .urg == 1 then "URG" else empty end),
    (if .ack == 1 then "ACK" else empty end),
    (if .psh == 1 then "PSH" else empty end),
    (if .rst == 1 then "RST" else empty end),
    (if .syn == 1 then "SYN" else empty end),
    (if .fin == 1 then "FIN" else empty end)
  ] | join(", ") as $active_flags |
  $root |
  "  TCP Segment Header:
    Source Port:      \(.src_port)
    Destination Port: \(.dst_port) (\(if .dst_port == 443 then "HTTPS" elif .dst_port == 80 then "HTTP" else "Other" end))
    Sequence Number:  \(.seq_num) (0x\(.seq_num | tostring))
    Ack Number:       \(.ack_num)
    Header Length:    \(.data_offset * 4) bytes (Offset: \(.data_offset))
    Active Flags:     [\($active_flags)]
    Window Size:      \(.window_size)
    Checksum:         0x\(.checksum | tostring)
    Urgent Pointer:   \(.urg_ptr)"
'

echo -e "\n--- Step 4: Multi-Packet Capture Dissection (${PACKETS_SAMPLE}) ---"

echo "Packet #1: DNS Query over UDP (Offset 0B, 32 bytes total)"
"${BDD}" --input-file="${PACKETS_SAMPLE}" \
        --input-skip-bits=0 \
        --count=1 \
        --preset=ipv4-header \
        --output-json \
        --json-object | \
jq -r "${JQ_IP_HELPER}"'
  "  [IP Layer]  \(.src_ip | to_ip) -> \(.dst_ip | to_ip) | Total Len: \(.total_length)B | Proto: UDP (\(.protocol))"
'
"${BDD}" --input-file="${PACKETS_SAMPLE}" \
        --input-skip-bits=20B \
        --count=1 \
        --preset=udp-header \
        --output-json \
        --json-object | \
jq -r '
  "  [UDP Layer] Port: \(.src_port) -> \(.dst_port) | Datagram Len: \(.length)B"
'
"${BDD}" --input-file="${PACKETS_SAMPLE}" \
        --input-skip-bits=28B \
        --input-pattern="dns_id:16U,flags:16U" \
        --count=1 \
        --output-json \
        --json-object | \
jq -r '
  "  [DNS Payload] Query ID: 0x\(.dns_id | tostring) | Flags: 0x\(.flags | tostring) (Standard Query)"
'

echo -e "\nPacket #2: TCP SYN Handshake (Offset 32B, 40 bytes total)"
"${BDD}" --input-file="${PACKETS_SAMPLE}" \
        --input-skip-bits=32B \
        --count=1 \
        --preset=ipv4-header \
        --output-json \
        --json-object | \
jq -r "${JQ_IP_HELPER}"'
  "  [IP Layer]  \(.src_ip | to_ip) -> \(.dst_ip | to_ip) | Total Len: \(.total_length)B | Proto: TCP (\(.protocol))"
'
"${BDD}" --input-file="${PACKETS_SAMPLE}" \
        --input-skip-bits=52B \
        --count=1 \
        --preset=tcp-header \
        --output-json \
        --json-object | \
jq -r '
  "  [TCP Layer] Port: \(.src_port) -> \(.dst_port) | Seq: \(.seq_num) | SYN: \(.syn) | Window: \(.window_size)"
'

echo -e "\nPacket #3: HTTP GET Request (Offset 72B, 58 bytes total)"
"${BDD}" --input-file="${PACKETS_SAMPLE}" \
        --input-skip-bits=72B \
        --count=1 \
        --preset=ipv4-header \
        --output-json \
        --json-object | \
jq -r "${JQ_IP_HELPER}"'
  "  [IP Layer]  \(.src_ip | to_ip) -> \(.dst_ip | to_ip) | Total Len: \(.total_length)B | Proto: TCP (\(.protocol))"
'
"${BDD}" --input-file="${PACKETS_SAMPLE}" \
        --input-skip-bits=92B \
        --count=1 \
        --preset=tcp-header \
        --output-json \
        --json-object | \
jq -r '
  "  [TCP Layer] Port: \(.src_port) -> \(.dst_port) | Seq: \(.seq_num) | ACK: \(.ack) | PSH: \(.psh)"
'
"${BDD}" --input-file="${PACKETS_SAMPLE}" \
        --input-skip-bits=112B \
        --input-pattern="payload:144C" \
        --count=1 \
        --output-json \
        --json-object | \
jq -r '
  "  [HTTP Payload] \(.payload | @json)"
'

echo -e "\nAll network header shell demonstrations completed successfully!"
