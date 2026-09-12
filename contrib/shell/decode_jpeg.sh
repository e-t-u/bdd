#!/usr/bin/env bash
# decode_jpeg.sh: Decode JPEG SOF0 (Start of Frame) geometry and 4-bit chroma nibbles
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BDD="${BASE_DIR}/target/release/bdd"
SAMPLE="${SCRIPT_DIR}/../data/sample.jpg"

if [ ! -x "${BDD}" ]; then
    BDD="bdd"
fi

INPUT_FILE="${1:-${SAMPLE}}"

echo "================================================================="
echo " JPEG SOF0 Geometry & 4-bit Chroma Subsampling Decoder"
echo " Pattern: 8U (Precision) 16U (Height) 16U (Width) 8U (Components)"
echo "          3 * (8U (ID) 4U (H-Factor) 4U (V-Factor) 8U (Q-Table))"
echo "================================================================="

# In sample.jpg, SOF0 payload starts at offset 24 bytes (after SOI, APP0, and SOF0 marker+length)
SOF_OFFSET=24

"${BDD}" "${SOF_OFFSET}B:120 -> 120" "8U16U16U8U3*(8U4U4U8U)" \
        --input-file="${INPUT_FILE}" \
        --count=1 \
        --output-json | \
jq -r '
  (if .[5] == 2 and .[6] == 2 then "4:2:0 (YUV 2x2 chroma subsampling)"
   elif .[5] == 2 and .[6] == 1 then "4:2:2"
   elif .[5] == 1 and .[6] == 1 then "4:4:4 (No subsampling)"
   else "Custom (\(.[5])x\(.[6]))" end) as $subsample |

  "• Image Geometry:\n" +
  "  Resolution:      \(.[2]) x \(.[1]) pixels\n" +
  "  Sample Depth:    \(.[0]) bits per sample\n" +
  "  Components:      \(.[3])\n" +
  "  Subsampling:     \($subsample)\n\n" +
  "• Color Component Breakdown (Unpacked 4-bit nibbles):\n" +
  "  Component 1 (Y) : ID=\(.[4]), H-Factor=\(.[5]), V-Factor=\(.[6]), QuantTable=\(.[7])\n" +
  "  Component 2 (Cb): ID=\(.[8]), H-Factor=\(.[9]), V-Factor=\(.[10]), QuantTable=\(.[11])\n" +
  "  Component 3 (Cr): ID=\(.[12]), H-Factor=\(.[13]), V-Factor=\(.[14]), QuantTable=\(.[15])"
'
