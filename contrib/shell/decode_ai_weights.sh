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
HEADER_LEN=$("${BDD}" "8*8U" \
                     --input-file="${SAFETENSORS}" \
                     --count=1 \
                     --output-json | \
            jq '.[0] + .[1]*256 + .[2]*65536 + .[3]*16777216')

DATA_OFFSET=$(( 8 + HEADER_LEN ))
printf "  JSON Header Length : %d bytes\n" "$HEADER_LEN"
printf "  Tensor Buffer Start: Byte %d (Seeking via stream pattern %dB:8 -> 8)\n" "$DATA_OFFSET" "$DATA_OFFSET"

echo -e "\n--- Step 2: Decoding OCP FP8 E4M3 Weights (8E) from Safetensors ---"
echo "Command: bdd \"${DATA_OFFSET}B:8 -> 8\" 8E --input-file=... --count=4 --output-json"
"${BDD}" "${DATA_OFFSET}B:8 -> 8" 8E \
        --input-file="${SAFETENSORS}" \
        --count=4 \
        --output-json | \
jq -r '"  FP8 Weight: " + (.[0]|tostring)'

echo -e "\n--- Step 3: Decoding Google Brain Bfloat16 Weights (16Y) from Safetensors ---"
BF16_OFFSET=$(( DATA_OFFSET + 4 ))
echo "Command: bdd \"${BF16_OFFSET}B:16 -> 16\" 16Y --input-file=... --input-little-endian --count=2 --output-json"
"${BDD}" "${BF16_OFFSET}B:16 -> 16" 16Y \
        --input-file="${SAFETENSORS}" \
        --input-little-endian \
        --count=2 \
        --output-json | \
jq -r '"  BF16 Weight: " + (.[0]|tostring)'

echo -e "\n--- Step 4: Decoding NVIDIA Blackwell NVFP4 (4E4E) Sub-Byte Weights ---"
echo "Each byte holds two 4-bit floats: [w0 (4E), w1 (4E)]"
echo "Command: bdd 4E4E --input-file=sample_nvfp4.bin --output-json"
"${BDD}" 4E4E \
        --input-file="${NVFP4_FILE}" \
        --count=4 \
        --output-json | \
jq -r '"  NVFP4 Pair: [w0=" + (.[0]|tostring) + ", w1=" + (.[1]|tostring) + "]"'

echo -e "\n--- Step 5: Decoding OCP Microscaling FP6 (4*6E) Unaligned Weights ---"
echo "4 weights of 6 bits each across 24 bits (3 bytes):"
echo "Command: bdd \"4*6E\" --input-file=sample_fp6.bin --output-json"
"${BDD}" "4*6E" \
        --input-file="${FP6_FILE}" \
        --output-json | \
jq -r '"  FP6 Quad: " + (tostring)'
