#!/usr/bin/env bash
# decode_wav.sh: Inspect RIFF WAV header, demux stereo channels, and process 24-bit PCM
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BDD="${BASE_DIR}/target/release/bdd"
SAMPLE_WAV="${SCRIPT_DIR}/../data/sample.wav"
SAMPLE_24="${SCRIPT_DIR}/../data/sample_24bit.raw"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "${TMP_DIR}"' EXIT

if [ ! -x "${BDD}" ]; then
    BDD="bdd"
fi

echo "================================================================="
echo " RIFF / WAV Audio Processing with bdd"
echo "================================================================="

echo -e "\n--- Step 1: Parsing 44-byte RIFF/WAVE Header ---"
echo "Pattern: 32C(RIFF) 32u(Size) 32C(WAVE) 32C(fmt ) 32u(FmtSize) 16u(AudioFmt)"
echo "         16u(Channels) 32u(SampleRate) 32u(ByteRate) 16u(BlockAlign)"
echo "         16u(BitsPerSample) 32C(data) 32u(DataSize)"

"${BDD}" --input-file="${SAMPLE_WAV}" \
        --input-pattern="32C32u32C32C32u16u16u32u32u16u16u32C32u" \
        --count=1 \
        --output-json | \
jq -r '
  "  Chunk ID:        \(.[0])\n" +
  "  File Size:       \(.[1] + 8) bytes\n" +
  "  Format:          \(.[2]) / \(.[3])\n" +
  "  Audio Format:    \(if .[5] == 1 then "PCM Uncompressed" else "Compressed (" + (.[5]|tostring) + ")" end)\n" +
  "  Channels:        \(.[6]) (\(if .[6] == 1 then "Mono" elif .[6] == 2 then "Stereo" else "Multichannel" end))\n" +
  "  Sample Rate:     \(.[7]) Hz\n" +
  "  Byte Rate:       \(.[8]) bytes/sec\n" +
  "  Bits Per Sample: \(.[10])-bit\n" +
  "  Data Chunk:      \(.[11]) (\(.[12]) payload bytes)"
'

echo -e "\n--- Step 2: Demuxing Interleaved 16-bit Stereo into Mono Channels ---"
echo "Command: bdd --input-skip-bits=352 --input-pattern=\"16S16S\" --demux 0:left.raw --demux 1:right.raw"
"${BDD}" --input-file="${SAMPLE_WAV}" \
        --input-skip-bits=352 \
        --input-pattern="16S16S" \
        --demux "0:${TMP_DIR}/left.raw" \
        --demux "1:${TMP_DIR}/right.raw" > /dev/null

echo "Left Channel first 5 signed samples:"
"${BDD}" --input-file="${TMP_DIR}/left.raw" --input-pattern="16S" --count=5 --output-tuples

echo "Right Channel first 5 signed samples:"
"${BDD}" --input-file="${TMP_DIR}/right.raw" --input-pattern="16S" --count=5 --output-tuples

echo -e "\n--- Step 3: Slicing Studio 24-bit PCM Audio to 16-bit Audio ---"
echo "Original 24-bit Signed Samples:"
"${BDD}" --input-file="${SAMPLE_24}" --input-pattern="24s" --output-tuples

echo "Downsampled to 16-bit (right-shifted 8 bits, output-unit 16):"
"${BDD}" --input-file="${SAMPLE_24}" \
        --input-pattern="24s" \
        --remove-right=0,8 \
        --output-unit=16 \
        --output-tuples
