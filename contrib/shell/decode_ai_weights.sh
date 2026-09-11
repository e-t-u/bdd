#!/usr/bin/env bash
# decode_ai_weights.sh: Inspect modern AI weights (Safetensors, NVFP4, FP6, FP8, BF16)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BDD="${BASE_DIR}/target/release/bdd"
DATA_DIR="${SCRIPT_DIR}/../data"

if [ ! -x "${BDD}" ]; then
    BDD="bdd"
fi

SAFETENSORS="${DATA_DIR}/sample.safetensors"
NVFP4_FILE="${DATA_DIR}/sample_nvfp4.bin"
FP6_FILE="${DATA_DIR}/sample_fp6.bin"

echo "================================================================="
echo " AI Model Weight Inspection with bdd"
echo "================================================================="

echo -e "\n--- Step 1: Reading Safetensors Header & Seeking Directly to Tensors ---"
# First 8 bytes is 64-bit LE header length
HEADER_LEN=$("${BDD}" --input-file="${SAFETENSORS}" \
                     --input-pattern="8*8U" \
                     --count=1 \
                     --output-json | \
            jq '.[0] + .[1]*256 + .[2]*65536 + .[3]*16777216')

DATA_OFFSET=$(( 8 + HEADER_LEN ))
printf "  JSON Header Length : %d bytes\n" "$HEADER_LEN"
printf "  Tensor Buffer Start: Byte %d (Seeking via --input-skip-bits=%dB)\n" "$DATA_OFFSET" "$DATA_OFFSET"

echo -e "\n--- Step 2: Decoding OCP FP8 E4M3 Weights (8E) from Safetensors ---"
echo "Command: bdd --input-skip-bits=${DATA_OFFSET}B --input-pattern=8E --count=4 --output-json"
"${BDD}" --input-file="${SAFETENSORS}" \
        --input-skip-bits="${DATA_OFFSET}B" \
        --input-pattern="8E" \
        --count=4 \
        --output-json | \
jq -r '"  FP8 Weight: " + (.[0]|tostring)'

echo -e "\n--- Step 3: Decoding Google Brain Bfloat16 Weights (16Y) from Safetensors ---"
BF16_OFFSET=$(( DATA_OFFSET + 4 ))
echo "Command: bdd --input-skip-bits=${BF16_OFFSET}B --input-little-endian --input-pattern=16Y --count=2 --output-json"
"${BDD}" --input-file="${SAFETENSORS}" \
        --input-skip-bits="${BF16_OFFSET}B" \
        --input-little-endian \
        --input-pattern="16Y" \
        --count=2 \
        --output-json | \
jq -r '"  BF16 Weight: " + (.[0]|tostring)'

echo -e "\n--- Step 4: Decoding NVIDIA Blackwell NVFP4 (4E4E) Sub-Byte Weights ---"
echo "Each byte holds two 4-bit floats: [w0 (4E), w1 (4E)]"
echo "Command: bdd --input-file=sample_nvfp4.bin --input-pattern=4E4E --output-json"
"${BDD}" --input-file="${NVFP4_FILE}" \
        --input-pattern="4E4E" \
        --count=4 \
        --output-json | \
jq -r '"  NVFP4 Pair: [w0=" + (.[0]|tostring) + ", w1=" + (.[1]|tostring) + "]"'

echo -e "\n--- Step 5: Decoding OCP Microscaling FP6 (4*6E) Unaligned Weights ---"
echo "4 weights of 6 bits each across 24 bits (3 bytes):"
echo "Command: bdd --input-file=sample_fp6.bin --input-pattern=\"4*6E\" --output-json"
"${BDD}" --input-file="${FP6_FILE}" \
        --input-pattern="4*6E" \
        --output-json | \
jq -r '"  FP6 Quad: " + (tostring)'
