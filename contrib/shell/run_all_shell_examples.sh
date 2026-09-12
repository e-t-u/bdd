#!/usr/bin/env bash
# run_all_shell_examples.sh: Run all bdd shell examples sequentially
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "#################################################################"
echo "# Running All bdd Shell Demonstrations"
echo "#################################################################"

"${SCRIPT_DIR}/decode_mp3.sh"
echo ""
"${SCRIPT_DIR}/decode_mpeg_ts.sh"
echo ""
"${SCRIPT_DIR}/decode_wav.sh"
echo ""
"${SCRIPT_DIR}/decode_mp4.sh"
echo ""
"${SCRIPT_DIR}/decode_jpeg.sh"
echo ""
"${SCRIPT_DIR}/decode_ai_weights.sh"
echo ""
"${SCRIPT_DIR}/decode_network.sh"

echo -e "\nAll shell script demonstrations completed successfully!"
