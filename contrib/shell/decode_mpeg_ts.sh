#!/usr/bin/env bash
# decode_mpeg_ts.sh: Inspect MPEG-2 Transport Stream 188-byte packets and 13-bit PIDs
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BDD="${BASE_DIR}/target/release/bdd"
SAMPLE="${SCRIPT_DIR}/../data/sample.ts"

if [ ! -x "${BDD}" ]; then
    BDD="bdd"
fi

INPUT_FILE="${1:-${SAMPLE}}"

echo "================================================================="
echo " MPEG Transport Stream Analysis with bdd: ${INPUT_FILE}"
echo " Stride: 188 bytes (1504 bits) per packet"
echo " Header Pattern: 8U (Sync 0x47) 1U (TEI) 1U (PUSI) 1U (Priority)"
echo "                 13U (PID) 2U (Scrambling) 2U (Adaptation) 4U (Counter)"
echo "================================================================="

echo -e "\n--- Step 1: Full Packet Header Decoding (First 4 Packets) ---"
"${BDD}" --input-file="${INPUT_FILE}" \
        --input-pattern="8U1U1U1U13U2U2U4U1472x" \
        --count=4 \
        --output-json | \
jq -r '
  (if .[4] == 0 then "PAT (Program Association Table)"
   elif .[4] == 256 then "H.264 Video Stream"
   elif .[4] == 257 then "AAC Audio Stream"
   else "Elementary Stream" end) as $desc |
  "• Packet #\(.[7]): Sync=0x\(.[0] | tostring) (0x47) | PID=\(.[4]) (0x\(.[4] | tostring)) [\($desc)] | PUSI=\(.[2]) | Continuity=\(.[7])"
'

echo -e "\n--- Step 2: Extracting 13-bit PIDs directly with Container Offset ---"
echo "Command: bdd --input-raw-unit=1504 --input-offset=11 --input-unit=13 --output-unit=13 --output-integers"
echo "Histogram of PIDs found in stream:"
"${BDD}" --input-file="${INPUT_FILE}" \
        --input-raw-unit=1504 \
        --input-offset=11 \
        --input-unit=13 \
        --output-unit=13 \
        --output-integers | \
sort -n | uniq -c | \
while read -r count pid; do
    case "$pid" in
        0) desc="PAT (Program Association Table)" ;;
        256) desc="H.264 Video" ;;
        257) desc="AAC Audio" ;;
        *) desc="Unknown" ;;
    esac
    printf "  PID %4d (0x%04x): %3d packet(s) [%s]\n" "$pid" "$pid" "$count" "$desc"
done
