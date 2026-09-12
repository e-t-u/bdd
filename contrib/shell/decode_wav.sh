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
echo "Pattern: 32C 4*8U 32C 32C 4*8U 2*8U 2*8U 4*8U 4*8U 2*8U 2*8U 32C 4*8U"

"${BDD}" --input-file="${SAMPLE_WAV}" \
        --input-pattern="32C4*8U32C32C4*8U2*8U2*8U4*8U4*8U2*8U2*8U32C4*8U" \
        --count=1 \
        --output-json | \
jq -r '
  def le16(b0; b1): b0 + b1 * 256;
  def le32(b0; b1; b2; b3): b0 + b1 * 256 + b2 * 65536 + b3 * 16777216;
  
  (le32(.[1]; .[2]; .[3]; .[4]) + 8) as $filesize |
  (le16(.[11]; .[12])) as $audio_fmt |
  (le16(.[13]; .[14])) as $channels |
  (le32(.[15]; .[16]; .[17]; .[18])) as $rate |
  (le32(.[19]; .[20]; .[21]; .[22])) as $byterate |
  (le16(.[25]; .[26])) as $bits |
  (le32(.[28]; .[29]; .[30]; .[31])) as $datasize |

  "  Chunk ID:        \(.[0])\n" +
  "  File Size:       \($filesize) bytes\n" +
  "  Format:          \(.[5]) / \(.[6])\n" +
  "  Audio Format:    \(if $audio_fmt == 1 then "PCM Uncompressed" else "Compressed" end)\n" +
  "  Channels:        \($channels) (\(if $channels == 2 then "Stereo" else "Mono" end))\n" +
  "  Sample Rate:     \($rate) Hz\n" +
  "  Byte Rate:       \($byterate) bytes/sec\n" +
  "  Bits Per Sample: \($bits)-bit\n" +
  "  Data Chunk:      \(.[27]) (\($datasize) payload bytes)"
'

echo -e "\n--- Step 2: Demuxing Interleaved 16-bit Stereo into Mono Channels ---"
echo "Command: bdd \"44B:32 -> 32\" 16S16S --input-little-endian --demux-files=left.raw,right.raw > /dev/null"
"${BDD}" --input-file="${SAMPLE_WAV}" \
        "44B:32 -> 32" \
        "16S16S" \
        --input-little-endian \
        --demux-files="${TMP_DIR}/left.raw,${TMP_DIR}/right.raw" > /dev/null

echo "Left Channel first 5 signed samples (440 Hz wave):"
"${BDD}" --input-file="${TMP_DIR}/left.raw" 16S --count=5 --output-tuples

echo "Right Channel first 5 signed samples (880 Hz wave):"
"${BDD}" --input-file="${TMP_DIR}/right.raw" 16S --count=5 --output-tuples

echo -e "\nTip: Play or convert raw PCM channels using ffmpeg / ffplay:"
echo "  ffplay -f s16le -ar 44100 -ac 1 -autoexit ${TMP_DIR}/left.raw"
echo "  ffplay -f s16le -ar 44100 -ac 1 -autoexit ${TMP_DIR}/right.raw"
echo "  ffmpeg -f s16le -ar 44100 -ac 1 -i ${TMP_DIR}/left.raw left.wav"
echo "  ffmpeg -f s16le -ar 44100 -ac 1 -i ${TMP_DIR}/right.raw right.wav"

echo -e "\n--- Step 3: Slicing Studio 24-bit PCM Audio to 16-bit Audio ---"
echo "Original 24-bit Signed Samples:"
"${BDD}" --input-file="${SAMPLE_24}" 24S --output-tuples

echo "Downsampled to 16-bit via container slicing (\"24[0:16] -> 16\" 16S):"
"${BDD}" --input-file="${SAMPLE_24}" \
        "24[0:16] -> 16" \
        16S \
        --output-tuples
