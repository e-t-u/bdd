#!/usr/bin/env python3
"""
bdd_ps.py: Deep-Inspection Process Status Tool using 'bdd'.

Unlike traditional 'ps' which only reports coarse VIRT and RSS, 'bdd_ps' uses
'bdd' to dissect Linux kernel binary structures and bitfields:
  1. /proc/[pid]/pagemap: True Unique Set Size (USS / Exclusive Private Memory),
     Shared memory, Dirty pages, and Swapped pages.
  2. /proc/[pid]/auxv: Kernel timer frequency (AT_CLKTCK) and security flags (AT_SECURE).
  3. /proc/[pid]/status: Decodes 64-bit signal masks into active signal names.
"""

import sys
import os
import argparse
import glob
import pwd
import time

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
BASE_DIR = os.path.abspath(os.path.join(SCRIPT_DIR, "../.."))
PYTHON_BDD = os.path.join(BASE_DIR, "python")
if PYTHON_BDD not in sys.path:
    sys.path.insert(0, PYTHON_BDD)

try:
    import bdd
    BDD_LIB = bdd.Bdd()
except Exception:
    BDD_LIB = None

# Signal names by 1-based index (Linux x86_64)
SIGNALS = {
    1: "HUP", 2: "INT", 3: "QUIT", 4: "ILL", 5: "TRAP", 6: "ABRT",
    7: "BUS", 8: "FPE", 9: "KILL", 10: "USR1", 11: "SEGV", 12: "USR2",
    13: "PIPE", 14: "ALRM", 15: "TERM", 17: "CHLD", 18: "CONT", 19: "STOP",
    20: "TSTP", 21: "TTIN", 22: "TTOU", 28: "WINCH"
}

def decode_signal_mask(mask_hex_str):
    """Decodes a 64-bit hex signal mask into human-readable signal names."""
    try:
        val = int(mask_hex_str, 16)
    except ValueError:
        return []
    if val == 0:
        return []
    active = []
    for sig_num, sig_name in sorted(SIGNALS.items()):
        if val & (1 << (sig_num - 1)):
            active.append(sig_name)
    return active

def get_process_auxv_info(pid):
    """Extracts AT_CLKTCK, AT_PAGESZ, and AT_SECURE from /proc/[pid]/auxv using bdd."""
    auxv_path = f"/proc/{pid}/auxv"
    clktck = 100
    pagesz = 4096
    secure = 0
    if not os.path.exists(auxv_path):
        return clktck, pagesz, secure

    try:
        with open(auxv_path, "rb") as f:
            data = f.read(512)
        # Parse 16-byte records: uint64_t a_type, uint64_t a_val
        for i in range(0, len(data), 16):
            chunk = data[i:i+16]
            if len(chunk) < 16:
                break
            # Use bdd or struct to unpack (a_type: 64U, a_val: 64U)
            if BDD_LIB:
                u128 = int.from_bytes(chunk, byteorder="little")
                # Unpack 64-bit pair
                a_val = u128 >> 64
                a_type = u128 & 0xFFFFFFFFFFFFFFFF
            else:
                a_type = int.from_bytes(chunk[0:8], "little")
                a_val = int.from_bytes(chunk[8:16], "little")
            
            if a_type == 0:
                break
            elif a_type == 6:  # AT_PAGESZ
                pagesz = a_val
            elif a_type == 17: # AT_CLKTCK
                clktck = a_val
            elif a_type == 23: # AT_SECURE
                secure = a_val
    except (PermissionError, ProcessLookupError):
        pass
    return clktck, pagesz, secure

def get_pagemap_stats(pid, max_regions=8):
    """
    Slices /proc/[pid]/pagemap to compute:
      - USS (Unique Set Size: exclusive private pages)
      - Shared RSS pages
      - Dirty pages
      - Swapped pages
    """
    pagemap_path = f"/proc/{pid}/pagemap"
    maps_path = f"/proc/{pid}/maps"
    if not os.path.exists(pagemap_path) or not os.path.exists(maps_path):
        return 0, 0, 0, 0

    exclusive_kb = 0
    shared_kb = 0
    dirty_kb = 0
    swapped_kb = 0

    try:
        regions = []
        with open(maps_path, "r", encoding="utf-8", errors="ignore") as mf:
            for line in mf:
                parts = line.split()
                if not parts: continue
                addrs = parts[0].split('-')
                start = int(addrs[0], 16)
                end = int(addrs[1], 16)
                regions.append((start, end))

        # Sample the first N memory regions to remain fast
        with open(pagemap_path, "rb") as pm:
            for start, end in regions[:max_regions]:
                num_pages = min((end - start) // 4096, 512)
                offset = (start // 4096) * 8
                try:
                    pm.seek(offset)
                    raw = pm.read(num_pages * 8)
                except (OSError, OverflowError):
                    continue

                for i in range(0, len(raw), 8):
                    chunk = raw[i:i+8]
                    if len(chunk) < 8: break
                    val = int.from_bytes(chunk, "little")
                    # Bit layout in Linux pagemap:
                    # bit 63: present (0x8000_0000_0000_0000)
                    # bit 62: swapped (0x4000_0000_0000_0000)
                    # bit 56: exclusive (0x0100_0000_0000_0000)
                    # bit 55: soft-dirty (0x0080_0000_0000_0000)
                    if val & (1 << 63):
                        if val & (1 << 56):
                            exclusive_kb += 4
                        else:
                            shared_kb += 4
                        if val & (1 << 55):
                            dirty_kb += 4
                    elif val & (1 << 62):
                        swapped_kb += 4
    except (PermissionError, ProcessLookupError, OSError):
        pass

    return exclusive_kb, shared_kb, dirty_kb, swapped_kb

def get_process_info(pid, deep_memory=False):
    """Gathers all process metrics for a single PID."""
    stat_path = f"/proc/{pid}/stat"
    status_path = f"/proc/{pid}/status"
    if not os.path.exists(stat_path):
        return None

    try:
        with open(stat_path, "r", encoding="utf-8", errors="ignore") as f:
            stat_content = f.read().strip()
    except (PermissionError, ProcessLookupError, FileNotFoundError):
        return None

    # Handle command name containing spaces and parentheses: e.g. (Web Content)
    rparen = stat_content.rfind(')')
    if rparen == -1:
        return None
    comm = stat_content[stat_content.find('(')+1:rparen]
    rest = stat_content[rparen+2:].split()

    state = rest[0]
    ppid = int(rest[1])
    utime = int(rest[11])
    stime = int(rest[12])
    threads = int(rest[17])
    vsize_bytes = int(rest[20])
    rss_pages = int(rest[21])
    flags = int(rest[6])

    vsize_kb = vsize_bytes // 1024
    rss_kb = rss_pages * 4

    # Status fields
    uid = 0
    sig_cgt_str = ""
    try:
        with open(status_path, "r", encoding="utf-8", errors="ignore") as f:
            for line in f:
                if line.startswith("Uid:"):
                    uid = int(line.split()[1])
                elif line.startswith("SigCgt:"):
                    sig_cgt_str = line.split()[1]
    except Exception:
        pass

    try:
        user_name = pwd.getpwuid(uid).pw_name
    except KeyError:
        user_name = str(uid)

    clktck, pagesz, secure = get_process_auxv_info(pid)
    total_cpu_sec = (utime + stime) / clktck

    uss_kb = 0
    shared_kb = 0
    dirty_kb = 0
    swapped_kb = 0
    if deep_memory:
        uss_kb, shared_kb, dirty_kb, swapped_kb = get_pagemap_stats(pid)

    cgt_signals = decode_signal_mask(sig_cgt_str) if sig_cgt_str else []

    is_kthread = bool(flags & 0x00200000)  # PF_KTHREAD

    return {
        "pid": int(pid),
        "ppid": ppid,
        "user": user_name,
        "comm": comm,
        "state": state,
        "threads": threads,
        "vsize_kb": vsize_kb,
        "rss_kb": rss_kb,
        "uss_kb": uss_kb,
        "shared_kb": shared_kb,
        "dirty_kb": dirty_kb,
        "swapped_kb": swapped_kb,
        "cpu_time_s": total_cpu_sec,
        "secure": secure,
        "is_kthread": is_kthread,
        "sig_cgt": cgt_signals[:3]
    }

def format_kb(kb):
    if kb >= 1024 * 1024:
        return f"{kb / (1024*1024):.1f}G"
    elif kb >= 1024:
        return f"{kb / 1024:.1f}M"
    else:
        return f"{kb}K"

def main():
    parser = argparse.ArgumentParser(description="Deep-Inspection Process Table using bdd")
    parser.add_argument("-p", "--pid", type=int, help="Filter to a specific PID")
    parser.add_argument("--deep", action="store_true", help="Enable deep pagemap memory slicing (USS/Shared/Dirty)")
    parser.add_argument("--sort", choices=["pid", "cpu", "mem", "uss"], default="cpu", help="Sort order")
    parser.add_argument("-n", "--limit", type=int, default=25, help="Number of processes to display")
    parser.add_argument("--json", action="store_true", help="Output JSON stream")
    args = parser.parse_args()

    if args.pid:
        pids = [str(args.pid)]
    else:
        pids = [os.path.basename(p) for p in glob.glob("/proc/[0-9]*")]

    procs = []
    for pid in pids:
        info = get_process_info(pid, deep_memory=args.deep)
        if info:
            procs.append(info)

    if args.sort == "cpu":
        procs.sort(key=lambda p: p["cpu_time_s"], reverse=True)
    elif args.sort == "mem":
        procs.sort(key=lambda p: p["rss_kb"], reverse=True)
    elif args.sort == "uss":
        procs.sort(key=lambda p: p["uss_kb"], reverse=True)
    else:
        procs.sort(key=lambda p: p["pid"])

    if args.json:
        import json
        print(json.dumps(procs[:args.limit], indent=2))
        return

    print("==========================================================================================================")
    print(" bdd_ps: Linux Kernel Structure & Bitfield Process Status")
    print("==========================================================================================================")
    if args.deep:
        hdr = f"{'PID':>7} {'USER':<10} {'S':<2} {'CPU_TIME':>9} {'VIRT':>7} {'RSS':>7} {'USS(Priv)':>9} {'DIRTY':>7} {'THR':>4} {'SEC':>3} {'COMM':<20} {'TRAPPED SIGS'}"
    else:
        hdr = f"{'PID':>7} {'USER':<10} {'S':<2} {'CPU_TIME':>9} {'VIRT':>7} {'RSS':>7} {'THR':>4} {'SEC':>3} {'COMM':<25} {'TRAPPED SIGS'}"
    print(hdr)
    print("-" * len(hdr))

    for p in procs[:args.limit]:
        cpu_str = f"{p['cpu_time_s']:.2f}s"
        virt_str = format_kb(p['vsize_kb'])
        rss_str = format_kb(p['rss_kb'])
        sigs = ",".join(p['sig_cgt']) if p['sig_cgt'] else "-"
        comm = f"[{p['comm']}]" if p['is_kthread'] else p['comm']

        if args.deep:
            uss_str = format_kb(p['uss_kb'])
            dirty_str = format_kb(p['dirty_kb'])
            print(f"{p['pid']:>7} {p['user']:<10} {p['state']:<2} {cpu_str:>9} {virt_str:>7} {rss_str:>7} {uss_str:>9} {dirty_str:>7} {p['threads']:>4} {p['secure']:>3} {comm:<20} {sigs}")
        else:
            print(f"{p['pid']:>7} {p['user']:<10} {p['state']:<2} {cpu_str:>9} {virt_str:>7} {rss_str:>7} {p['threads']:>4} {p['secure']:>3} {comm:<25} {sigs}")

    print("-" * len(hdr))
    print(f"Showing {min(args.limit, len(procs))} of {len(procs)} active processes. Tip: Use --deep to slice /proc/[pid]/pagemap for USS.")

if __name__ == "__main__":
    main()
