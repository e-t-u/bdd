#!/usr/bin/env bash
# stream_memory_keys.sh: Stream system raw memory 256 bits at a time and probe cryptographic keys using bdd
# Demonstrates:
#   1. Streaming memory in units of 256 bits (32 bytes = 1 unit)
#   2. Localized sliding-window Shannon entropy scanning across memory
#   3. Discovery of potential AES-256, ChaCha20, Ed25519, and SHA-256 keys
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
if [ -n "${BDD_BIN:-}" ] && [ -x "${BDD_BIN}" ]; then
    BDD="${BDD_BIN}"
elif [ -x "${BASE_DIR}/target/release/bdd" ]; then
    BDD="${BASE_DIR}/target/release/bdd"
elif [ -x "${BASE_DIR}/target/debug/bdd" ]; then
    BDD="${BASE_DIR}/target/debug/bdd"
elif [ -x "${BASE_DIR}/bdd" ]; then
    BDD="${BASE_DIR}/bdd"
else
    BDD="bdd"
fi

SOURCE="auto"
UNITS=4096          # 4096 * 32 bytes = 128 KiB
KEY_SIZE=256        # 256-bit keys (32 bytes)
OFFSET=0
JSON_OUTPUT=0
TARGET_PID=""

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Stream system raw memory 256 bits at a time into bdd and probe for
potential maximum-entropy cryptographic keys (AES-256, ChaCha20, Ed25519).

Options:
  -s, --source <SOURCE>    Memory source:
                             auto       - Kernel memory if root/sudo, else process memory [default]
                             kcore      - Linux kernel memory (/proc/kcore)
                             devmem     - Physical RAM (/dev/mem)
                             self       - Current process memory space (/proc/self/mem)
                             pid <PID>  - Specific process memory space (/proc/<PID>/mem)
                             demo       - In-memory cryptographic key demo (NIST AES-256 & ChaCha20)
                             <FILE>     - Raw memory dump or image file
  -n, --units <COUNT>      Number of 256-bit units to stream (default: 4096 = 128 KiB)
  -k, --key-size <BITS>    Candidate key size in bits (default: 256)
  -o, --offset <BYTES>     Byte offset to begin streaming from (default: 0)
  -j, --json               Output raw JSON probe report from bdd
  -h, --help               Show this help message and exit

Examples:
  # Auto-detect and stream 4096 units (256-bit each) to probe for 256-bit keys:
  $0

  # Run in-memory demonstration with real NIST test keys:
  $0 --source demo

  # Stream kernel memory starting from offset 1 MB:
  $0 --source kcore --offset 1048576 --units 8192

  # Stream memory of a specific daemon or process:
  $0 --source pid 1234 --units 2048

  # Output structured JSON metrics:
  $0 --json
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -s|--source)
            SOURCE="$2"
            if [ "$SOURCE" = "pid" ] || [ "$SOURCE" = "process" ]; then
                shift
                TARGET_PID="$2"
            fi
            shift 2
            ;;
        -n|--units)
            UNITS="$2"
            shift 2
            ;;
        -k|--key-size)
            KEY_SIZE="$2"
            shift 2
            ;;
        -o|--offset)
            OFFSET="$2"
            shift 2
            ;;
        -j|--json)
            JSON_OUTPUT=1
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            if [ -f "$1" ]; then
                SOURCE="$1"
                shift
            else
                echo "Unknown option: $1" >&2
                usage
            fi
            ;;
    esac
done

# Resolve auto source
if [ "$SOURCE" = "auto" ]; then
    if [ "$EUID" -eq 0 ] || sudo -n true 2>/dev/null; then
        SOURCE="kcore"
    else
        SOURCE="self"
    fi
fi

if [ "$JSON_OUTPUT" -eq 0 ]; then
    echo "========================================================================"
    echo " Streaming System Raw Memory (256 bits/unit) into bdd Key Prober"
    echo " Source:       ${SOURCE}${TARGET_PID:+ (PID: $TARGET_PID)}"
    echo " Unit Width:   256 bits (32 bytes per unit)"
    echo " Units Count:  ${UNITS} units ($(( UNITS * 32 )) bytes / $(( UNITS * 256 )) bits)"
    echo " Key Target:   ${KEY_SIZE} bits ($(( KEY_SIZE / 8 )) bytes)"
    echo " Memory Start: 0x$(printf '%X' "$OFFSET") ($OFFSET bytes)"
    echo "========================================================================"
fi

# Stream generator function
stream_memory() {
    case "$SOURCE" in
        kcore)
            local skip_units=$(( OFFSET / 32 ))
            if [ "$EUID" -eq 0 ]; then
                dd if=/proc/kcore bs=32 skip="$skip_units" count="$UNITS" status=none 2>/dev/null || true
            else
                sudo -n dd if=/proc/kcore bs=32 skip="$skip_units" count="$UNITS" status=none 2>/dev/null || true
            fi
            ;;
        devmem)
            local skip_units=$(( OFFSET / 32 ))
            if [ "$EUID" -eq 0 ]; then
                dd if=/dev/mem bs=32 skip="$skip_units" count="$UNITS" status=none 2>/dev/null || true
            else
                sudo -n dd if=/dev/mem bs=32 skip="$skip_units" count="$UNITS" status=none 2>/dev/null || true
            fi
            ;;
        self|pid|process)
            local pid="${TARGET_PID:-self}"
            python3 -c '
import sys, os

pid = "'"$pid"'"
target_units = int("'"$UNITS"'")
target_bytes = target_units * 32
start_offset = int("'"$OFFSET"'")

maps_file = f"/proc/{pid}/maps"
mem_file = f"/proc/{pid}/mem"

if not os.path.exists(maps_file):
    sys.exit(f"Error: Process maps {maps_file} does not exist")

written = 0
try:
    with open(mem_file, "rb", 0) as mem:
        with open(maps_file, "r") as maps:
            for line in maps:
                parts = line.split()
                if len(parts) < 2:
                    continue
                perms = parts[1]
                # Read readable mapped regions (heap, stack, anonymous mmap, data)
                if not perms.startswith("r"):
                    continue
                addr_range = parts[0].split("-")
                start_addr = int(addr_range[0], 16)
                end_addr = int(addr_range[1], 16)
                region_len = end_addr - start_addr

                if start_offset > 0:
                    if start_offset >= region_len:
                        start_offset -= region_len
                        continue
                    start_addr += start_offset
                    region_len -= start_offset
                    start_offset = 0

                chunk_len = min(region_len, target_bytes - written)
                if chunk_len <= 0:
                    break

                try:
                    mem.seek(start_addr)
                    data = mem.read(chunk_len)
                    if data:
                        sys.stdout.buffer.write(data)
                        written += len(data)
                except (OSError, IOError):
                    pass

                if written >= target_bytes:
                    break
except Exception as e:
    sys.stderr.write(f"Warning: memory stream error: {e}\n")
'
            ;;
        demo)
            python3 -c '
import sys

target_units = int("'"$UNITS"'")
# 1. Zero padding (approx 25% of units)
p1_units = max(8, target_units // 4)
sys.stdout.buffer.write(b"\x00" * (p1_units * 32))

# 2. Known NIST AES-256 test key
aes_key = bytes.fromhex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4")
sys.stdout.buffer.write(aes_key)

# 3. Structured ASCII / text records
sys.stdout.buffer.write(b"USER=admin\x00HOST=127.0.0.1\x00PORT=443\x00ENV=prod\x00" * 16)

# 4. Known ChaCha20 256-bit test key
chacha_key = bytes.fromhex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
sys.stdout.buffer.write(chacha_key)

# 5. Remaining zero padding
written_bytes = (p1_units * 32) + 32 + (16 * 44) + 32
rem_bytes = max(0, (target_units * 32) - written_bytes)
sys.stdout.buffer.write(b"\x00" * rem_bytes)
'
            ;;
        *)
            if [ -f "$SOURCE" ]; then
                local skip_units=$(( OFFSET / 32 ))
                dd if="$SOURCE" bs=32 skip="$skip_units" count="$UNITS" status=none 2>/dev/null || true
            else
                echo "Error: Unknown or inaccessible source '$SOURCE'" >&2
                exit 1
            fi
            ;;
    esac
}

BDD_FLAGS=(
    "--input-unit=256"
    "--probe-keys=${KEY_SIZE}"
)

if [ "$JSON_OUTPUT" -eq 1 ]; then
    BDD_FLAGS+=("--output-json")
fi

# Stream raw memory into bdd
stream_memory | "${BDD}" "${BDD_FLAGS[@]}"
