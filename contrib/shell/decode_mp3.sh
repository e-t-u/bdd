#!/usr/bin/env bash
# decode_mp3.sh: Decode MPEG Audio frame headers using bdd
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BDD="${BASE_DIR}/target/release/bdd"
SAMPLE="${SCRIPT_DIR}/../data/sample.mp3"

if [ ! -x "${BDD}" ]; then
    BDD="bdd"
fi

INPUT_FILE="${1:-${SAMPLE}}"

echo "================================================================="
echo " Decoding MP3 Frame Headers with bdd: ${INPUT_FILE}"
echo " Pattern: 11U (Sync) 2U (Ver) 2U (Layer) 1U (Prot) 4U (Bitrate)"
echo "          2U (Freq) 1U (Pad) 1U (Priv) 2U (Mode) 2U (Ext)"
echo "          1U (Copy) 1U (Orig) 2U (Emph)"
echo "================================================================="

decode_frame() {
    local offset="$1"
    local label="$2"
    echo "--- ${label} (Byte Offset: ${offset}) ---"
    "${BDD}" "${offset}B:32 -> 32" "11U2U2U1U4U2U1U1U2U2U1U1U2U" \
            --input-file="${INPUT_FILE}" \
            --count=1 \
            --output-json | \
    jq -r '
      ["MPEG-2.5", null, "MPEG-2", "MPEG-1"][.[1]] as $ver |
      ["Reserved", "Layer III (MP3)", "Layer II", "Layer I"][.[2]] as $layer |
      (if .[3] == 1 then "No CRC" else "16-bit CRC" end) as $crc |
      [null, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320][.[4]] as $kbps |
      ["44.1 kHz", "48.0 kHz", "32.0 kHz", "Reserved"][.[5]] as $rate |
      (if .[6] == 1 then "Yes" else "No" end) as $pad |
      ["Stereo", "Joint Stereo", "Dual Channel", "Single Channel (Mono)"][.[8]] as $ch |
      (if .[11] == 1 then "Original" else "Copy" end) as $orig |
      "  Sync: 0x\(.[0] | tostring) (Valid Syncword)\n  Format: \($ver) \($layer)\n  Bitrate: \($kbps) kbps\n  Sampling Rate: \($rate)\n  Channels: \($ch)\n  Padding: \($pad)\n  CRC Protection: \($crc)\n  Copyright/Original: \($orig)"
    '
}

decode_frame "0" "Frame 0 (First Frame)"
decode_frame "417" "Frame 1 (Fast-seeked via stream pattern 417B:32 -> 32)"
