#!/usr/bin/env bash
# decode_mp4.sh: Traverse MP4 ISO-BMFF boxes and unpack H.264 NAL bitfields
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BDD="${BASE_DIR}/target/release/bdd"
SAMPLE="${SCRIPT_DIR}/../data/sample.mp4"

if [ ! -x "${BDD}" ]; then
    BDD="bdd"
fi

INPUT_FILE="${1:-${SAMPLE}}"

echo "================================================================="
echo " MP4 / ISO-BMFF Box & H.264 NAL Bitfield Analysis with bdd"
echo "================================================================="

echo -e "\n--- Step 1: Traversing Top-Level MP4 Boxes ---"
echo "Box Header Pattern: 32U (Box Size in bytes) 32C (FourCC Type)"

OFFSET=0
FILE_SIZE=$(stat -c%s "${INPUT_FILE}")

while [ "$OFFSET" -lt "$FILE_SIZE" ]; do
    BOX_JSON=$("${BDD}" --input-file="${INPUT_FILE}" \
                       --input-skip-bits="${OFFSET}B" \
                       --input-pattern="32U32C" \
                       --count=1 \
                       --output-json)
    
    BOX_SIZE=$(echo "$BOX_JSON" | jq -r '.[0]')
    BOX_TYPE=$(echo "$BOX_JSON" | jq -r '.[1]')
    
    printf "• Box at offset %3d: Type='%s' | Size=%d bytes\n" "$OFFSET" "$BOX_TYPE" "$BOX_SIZE"
    
    if [ "$BOX_SIZE" -le 0 ]; then
        break
    fi
    OFFSET=$(( OFFSET + BOX_SIZE ))
done

echo -e "\n--- Step 2: Unpacking H.264 NAL Units inside 'mdat' Payload ---"
echo "NAL Pattern: 32U (Length) 1U (Forbidden Zero) 2U (Ref IDC) 5U (Unit Type)"

# In our sample, mdat is at offset 28; payload starts at 28 + 8 = 36 bytes
NAL_OFFSET=36
MDAT_END=68

while [ "$NAL_OFFSET" -lt "$MDAT_END" ]; do
    NAL_JSON=$("${BDD}" --input-file="${INPUT_FILE}" \
                       --input-skip-bits="${NAL_OFFSET}B" \
                       --input-pattern="32U1U2U5U" \
                       --count=1 \
                       --output-json)
    
    LEN=$(echo "$NAL_JSON" | jq -r '.[0]')
    FORBIDDEN=$(echo "$NAL_JSON" | jq -r '.[1]')
    REF_IDC=$(echo "$NAL_JSON" | jq -r '.[2]')
    TYPE=$(echo "$NAL_JSON" | jq -r '.[3]')

    case "$TYPE" in
        1) DESC="Non-IDR Coded Slice" ;;
        5) DESC="IDR Keyframe" ;;
        7) DESC="SPS (Sequence Parameter Set)" ;;
        8) DESC="PPS (Picture Parameter Set)" ;;
        *) DESC="Other NAL Unit" ;;
    esac

    printf "  NAL at %2dB: Type %d [%-27s] | Ref IDC: %d | Len: %d bytes (Forbidden: %d)\n" \
           "$NAL_OFFSET" "$TYPE" "$DESC" "$REF_IDC" "$LEN" "$FORBIDDEN"

    NAL_OFFSET=$(( NAL_OFFSET + 4 + LEN ))
done
