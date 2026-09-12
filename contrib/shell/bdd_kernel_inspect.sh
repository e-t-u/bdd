#!/usr/bin/env bash
# bdd_kernel_inspect.sh: Inspect Linux kernel structures in /proc and /sys using bdd
# Demonstrates bit-level slicing of:
#   1. ELF Auxiliary Vectors (/proc/[pid]/auxv)
#   2. Virtual Memory Page Tables (/proc/[pid]/pagemap)
#   3. PCI Device Hardware Config Space (/sys/bus/pci/devices/*/config)
#   4. Hex-encoded Network Sockets (/proc/net/tcp)
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

TARGET_PID="${1:-$$}"

echo "========================================================================"
echo " Linux Kernel Structure Inspection with bdd"
echo " Target PID: ${TARGET_PID} ($(cat /proc/${TARGET_PID}/comm 2>/dev/null || echo "unknown"))"
echo "========================================================================"

# -----------------------------------------------------------------------------
# 1. ELF Auxiliary Vectors (/proc/[pid]/auxv)
# -----------------------------------------------------------------------------
echo -e "\n--- 1. ELF Auxiliary Vectors (/proc/${TARGET_PID}/auxv) ---"
echo "Kernel binary structure: Elf64_auxv_t { uint64_t a_type; uint64_t a_val; }"
echo "bdd pattern: 'val:64U,type:64U' with --input-little-endian"

"${BDD}" --input-file="/proc/${TARGET_PID}/auxv" \
        --input-little-endian \
        "val:64U,type:64U" \
        --output-json 2>/dev/null | jq -r '
  def auxv_name(t):
    if t == 3 then "AT_PHDR    (Program Headers)"
    elif t == 4 then "AT_PHENT   (Size of Header)"
    elif t == 5 then "AT_PHNUM   (Number of Headers)"
    elif t == 6 then "AT_PAGESZ  (Page Size Bytes)"
    elif t == 7 then "AT_BASE    (Interpreter Base)"
    elif t == 9 then "AT_ENTRY   (ELF Entry Point)"
    elif t == 11 then "AT_UID     (Real UID)"
    elif t == 12 then "AT_EUID    (Effective UID)"
    elif t == 13 then "AT_GID     (Real GID)"
    elif t == 14 then "AT_EGID    (Effective GID)"
    elif t == 16 then "AT_HWCAP   (CPU Capabilities)"
    elif t == 17 then "AT_CLKTCK  (Clock Ticks/Sec)"
    elif t == 23 then "AT_SECURE  (Secure/SUID Flag)"
    elif t == 31 then "AT_EXECFN  (Path to Executable)"
    elif t == 33 then "AT_SYSINFO (vDSO Address)"
    else "AT_\(t)" end;

  if .type != 0 and .type != 15 and .type != 25 and .type != 26 and .type != 27 and .type != 28 and .type != 51 then
    "\(auxv_name(.type))\t\(.val)"
  else empty end
' | while IFS=$'\t' read -r k v; do
    printf "  %-35s : %s\n" "$k" "$v"
done | sort

# -----------------------------------------------------------------------------
# 2. Virtual Memory Page Table Slicing (/proc/[pid]/pagemap)
# -----------------------------------------------------------------------------
echo -e "\n--- 2. Virtual Memory Page Table Slicing (/proc/${TARGET_PID}/pagemap) ---"
echo "Kernel binary structure: 64-bit bitfield per 4096-byte virtual page"
echo "Bit layout: present(1b), swapped(1b), file_page(1b), reserved(3x), uffd(1b), exclusive(1b), soft_dirty(1b), pfn(55u)"

STACK_LINE=$(grep -E '\[stack\]' "/proc/${TARGET_PID}/maps" | head -n 1 || true)
if [ -n "${STACK_LINE}" ]; then
    END_HEX=$(echo "${STACK_LINE}" | awk '{print $1}' | cut -d'-' -f2)
    # Convert hex to decimal
    END_ADDR=$((16#${END_HEX}))
    # Stack grows downwards; inspect the top 4 active pages
    START_PAGE=$(( (END_ADDR / 4096) - 4 ))
    OFFSET_BITS=$(( START_PAGE * 8 * 8 ))

    echo "Slicing top 4 stack pages at offset bit ${OFFSET_BITS} (O(1) kernel seek):"
    "${BDD}" --input-file="/proc/${TARGET_PID}/pagemap" \
            --input-skip-bits="${OFFSET_BITS}" \
            --input-little-endian \
            "present:1b,swapped:1b,file_page:1b,3x,uffd_wp:1b,exclusive:1b,soft_dirty:1b,pfn:55u" \
            --count=4 \
            --output-json 2>/dev/null | jq -r '
      "  Page: present=\(.present) swapped=\(.swapped) exclusive=\(.exclusive) soft_dirty=\(.soft_dirty) pfn=\(.pfn)"
    '
else
    echo "  [stack] region not found or not accessible."
fi

# -----------------------------------------------------------------------------
# 3. PCI Device Hardware Config Space (/sys/bus/pci/devices/*/config)
# -----------------------------------------------------------------------------
echo -e "\n--- 3. PCI Device Hardware Config Space (/sys/bus/pci/devices/*/config) ---"
echo "Kernel binary structure: 256-byte standard PCI config header"
echo "Pattern: bar0:32U, bist:8U, hdr_type:8U, latency:8U, cache_line:8U, class:8U, subclass:8U, prog_if:8U, rev:8U, status:16U, cmd:16U, dev:16U, vendor:16U"

printf "  %-14s %-10s %-10s %-10s %-10s %s\n" "PCI SLOT" "VENDOR" "DEVICE" "CLASS" "HDR TYPE" "STATUS"
count=0
for cfg in /sys/bus/pci/devices/*/config; do
    [ -r "${cfg}" ] || continue
    slot=$(basename "$(dirname "${cfg}")")
    line=$("${BDD}" --input-file="${cfg}" \
            --input-little-endian \
            "bar0:32U,bist:8U,hdr_type:8U,latency:8U,cache_line:8U,class_code:8U,subclass:8U,prog_if:8U,rev_id:8U,status:16U,cmd:16U,device_id:16U,vendor_id:16U" \
            --count=1 \
            --output-json 2>/dev/null | jq -r '
      "0x\(.vendor_id | tostring)\t0x\(.device_id | tostring)\t0x\(.class_code | tostring)\(.subclass | tostring)\t0x\(.hdr_type | tostring)\t0x\(.status | tostring)"
    ' || true)
    if [ -n "${line}" ]; then
        IFS=$'\t' read -r v d c h s <<< "${line}"
        printf "  %-14s %-10s %-10s %-10s %-10s %s\n" "$slot" "$v" "$d" "$c" "$h" "$s"
        count=$((count + 1))
        [ $count -ge 6 ] && break
    fi
done

echo -e "\n========================================================================"
echo " Inspection Complete."
echo "========================================================================"
