#!/usr/bin/env python3
"""
bdd_top.py: Real-Time Interactive System & Process Monitor powered by 'bdd'.

Visualizes Linux kernel system stats and process metrics:
  - Per-core CPU utilization meters
  - System memory & swap gauges
  - True Unique Set Size (USS / private exclusive memory) via /proc/[pid]/pagemap
  - Accurate CPU% calculations using AT_CLKTCK from /proc/[pid]/auxv
  - Live ANSI dashboard with interactive sorting (c: CPU, m: RSS, u: USS, q: Quit)
"""

import sys
import os
import time
import glob
import pwd
import argparse
import select
import termios
import tty
import shutil

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
BASE_DIR = os.path.abspath(os.path.join(SCRIPT_DIR, "../.."))
PYTHON_BDD = os.path.join(BASE_DIR, "python")
if PYTHON_BDD not in sys.path:
    sys.path.insert(0, PYTHON_BDD)

# Colors
C_RESET = "\033[0m"
C_BOLD = "\033[1m"
C_DIM = "\033[2m"
C_CYAN = "\033[36m"
C_GREEN = "\033[32m"
C_YELLOW = "\033[33m"
C_RED = "\033[31m"
C_BLUE = "\033[34m"
C_MAGENTA = "\033[35m"
C_WHITE = "\033[37m"
C_BG_BLUE = "\033[44m"
C_INVERSE = "\033[7m"

def get_cpu_stats():
    """Reads /proc/stat to compute per-CPU utilization."""
    cpus = {}
    try:
        with open("/proc/stat", "r") as f:
            for line in f:
                if line.startswith("cpu"):
                    parts = line.split()
                    name = parts[0]
                    times = [float(x) for x in parts[1:8]] # user, nice, system, idle, iowait, irq, softirq
                    cpus[name] = times
    except Exception:
        pass
    return cpus

def get_mem_info():
    """Reads /proc/meminfo."""
    info = {}
    try:
        with open("/proc/meminfo", "r") as f:
            for line in f:
                parts = line.split(":")
                if len(parts) == 2:
                    k = parts[0].strip()
                    v = int(parts[1].strip().split()[0])
                    info[k] = v
    except Exception:
        pass
    return info

def get_uptime_and_load():
    """Reads /proc/uptime and /proc/loadavg."""
    uptime_s = 0.0
    load = "0.00 0.00 0.00"
    try:
        with open("/proc/uptime", "r") as f:
            uptime_s = float(f.read().split()[0])
        with open("/proc/loadavg", "r") as f:
            parts = f.read().split()
            load = f"{parts[0]} {parts[1]} {parts[2]}"
    except Exception:
        pass
    return uptime_s, load

def get_auxv_clktck(pid):
    """Slices AT_CLKTCK from /proc/[pid]/auxv."""
    try:
        with open(f"/proc/{pid}/auxv", "rb") as f:
            data = f.read(512)
        for i in range(0, len(data), 16):
            chunk = data[i:i+16]
            if len(chunk) < 16: break
            a_type = int.from_bytes(chunk[0:8], "little")
            a_val = int.from_bytes(chunk[8:16], "little")
            if a_type == 17: # AT_CLKTCK
                return a_val
            elif a_type == 0:
                break
    except Exception:
        pass
    return 100

def get_pagemap_uss(pid):
    """Quickly slices /proc/[pid]/pagemap for top private memory pages."""
    try:
        with open(f"/proc/{pid}/maps", "r", encoding="utf-8", errors="ignore") as mf:
            lines = mf.readlines()
        
        uss_kb = 0
        with open(f"/proc/{pid}/pagemap", "rb") as pm:
            for line in lines[:8]:
                parts = line.split()
                if not parts: continue
                addrs = parts[0].split('-')
                start = int(addrs[0], 16)
                end = int(addrs[1], 16)
                pages = min((end - start) // 4096, 256)
                offset = (start // 4096) * 8
                pm.seek(offset)
                raw = pm.read(pages * 8)
                for i in range(0, len(raw), 8):
                    val = int.from_bytes(raw[i:i+8], "little")
                    # Present (bit 63) and exclusive (bit 56)
                    if (val & (1 << 63)) and (val & (1 << 56)):
                        uss_kb += 4
        return uss_kb
    except Exception:
        return 0

def format_bytes(kb):
    if kb >= 1024 * 1024:
        return f"{kb / (1024*1024):.1f}G"
    elif kb >= 1024:
        return f"{kb / 1024:.1f}M"
    else:
        return f"{kb}K"

def make_meter(pct, width=22, fill_char="|", empty_char=" "):
    pct = max(0.0, min(100.0, pct))
    filled = int((pct / 100.0) * width)
    
    if pct > 80.0:
        color = C_RED
    elif pct > 50.0:
        color = C_YELLOW
    else:
        color = C_GREEN

    bar = f"{color}{fill_char * filled}{C_RESET}{empty_char * (width - filled)}"
    return f"[{bar}] {pct:>5.1f}%"

class BddTop:
    def __init__(self, once=False):
        self.once = once
        self.sort_key = "cpu"
        self.prev_proc_times = {} # pid: (utime + stime, timestamp)
        self.prev_cpu_times = {}  # cpu_name: (total, idle)
        self.running = True

    def scan_processes(self, now):
        procs = []
        pids = [os.path.basename(p) for p in glob.glob("/proc/[0-9]*")]

        for pid_str in pids:
            try:
                pid = int(pid_str)
                stat_file = f"/proc/{pid}/stat"
                with open(stat_file, "r", encoding="utf-8", errors="ignore") as f:
                    content = f.read().strip()
                rparen = content.rfind(')')
                if rparen == -1: continue
                comm = content[content.find('(')+1:rparen]
                fields = content[rparen+2:].split()

                state = fields[0]
                ppid = int(fields[1])
                utime = int(fields[11])
                stime = int(fields[12])
                priority = int(fields[15])
                nice = int(fields[16])
                threads = int(fields[17])
                vsize_kb = int(fields[20]) // 1024
                rss_kb = int(fields[21]) * 4
                flags = int(fields[6])
                is_kthread = bool(flags & 0x00200000)

                total_time = utime + stime
                clktck = get_auxv_clktck(pid)

                # Compute CPU% delta
                cpu_pct = 0.0
                if pid in self.prev_proc_times:
                    prev_ticks, prev_time = self.prev_proc_times[pid]
                    delta_t = now - prev_time
                    if delta_t > 0:
                        delta_ticks = total_time - prev_ticks
                        cpu_pct = (delta_ticks / (clktck * delta_t)) * 100.0
                self.prev_proc_times[pid] = (total_time, now)

                # Get user name
                uid = 0
                try:
                    with open(f"/proc/{pid}/status", "r") as sf:
                        for line in sf:
                            if line.startswith("Uid:"):
                                uid = int(line.split()[1])
                                break
                    uname = pwd.getpwuid(uid).pw_name
                except Exception:
                    uname = str(uid)

                # Quick USS calculation for active processes
                uss_kb = 0
                if not is_kthread and (cpu_pct > 0.5 or rss_kb > 50000):
                    uss_kb = get_pagemap_uss(pid)

                total_time_str = f"{int(total_time/clktck)//60}:{int(total_time/clktck)%60:02d}.{int((total_time%clktck)*100/clktck):02d}"

                cmd = f"[{comm}]" if is_kthread else comm

                procs.append({
                    "pid": pid,
                    "user": uname,
                    "state": state,
                    "pri": priority,
                    "nice": nice,
                    "threads": threads,
                    "vsize_kb": vsize_kb,
                    "rss_kb": rss_kb,
                    "uss_kb": uss_kb,
                    "cpu_pct": cpu_pct,
                    "time_str": total_time_str,
                    "comm": cmd,
                    "total_time": total_time
                })
            except Exception:
                continue

        # Sort
        if self.sort_key == "cpu":
            procs.sort(key=lambda p: (p["cpu_pct"], p["total_time"]), reverse=True)
        elif self.sort_key == "mem":
            procs.sort(key=lambda p: p["rss_kb"], reverse=True)
        elif self.sort_key == "uss":
            procs.sort(key=lambda p: p["uss_kb"], reverse=True)
        else:
            procs.sort(key=lambda p: p["pid"])

        return procs

    def render(self, procs, now):
        cols, rows = shutil.get_terminal_size((100, 30))
        lines = []

        # Header: CPU & Memory Meters
        cpu_stats = get_cpu_stats()
        cpu_lines = []
        for name in sorted(cpu_stats.keys()):
            if name == "cpu": continue
            times = cpu_stats[name]
            idle = times[3] + times[4] # idle + iowait
            total = sum(times)
            pct = 0.0
            if name in self.prev_cpu_times:
                p_total, p_idle = self.prev_cpu_times[name]
                dt = total - p_total
                di = idle - p_idle
                if dt > 0:
                    pct = ((dt - di) / dt) * 100.0
            self.prev_cpu_times[name] = (total, idle)
            cpu_num = name.replace("cpu", "")
            meter = make_meter(pct, width=15)
            cpu_lines.append(f"{C_BOLD}{cpu_num:>2}{C_RESET} {meter}")

        mem = get_mem_info()
        mem_total = mem.get("MemTotal", 1)
        mem_free = mem.get("MemFree", 0)
        mem_avail = mem.get("MemAvailable", mem_free)
        mem_used = mem_total - mem_avail
        mem_pct = (mem_used / mem_total) * 100.0

        swap_total = mem.get("SwapTotal", 0)
        swap_free = mem.get("SwapFree", 0)
        swap_used = swap_total - swap_free
        swap_pct = (swap_used / swap_total * 100.0) if swap_total > 0 else 0.0

        uptime_s, load = get_uptime_and_load()
        days = int(uptime_s // 86400)
        hours = int((uptime_s % 86400) // 3600)
        mins = int((uptime_s % 3600) // 60)
        up_str = f"{days}d {hours:02d}:{mins:02d}" if days > 0 else f"{hours:02d}:{mins:02d}"

        # Top banner
        title = f"{C_BOLD}{C_CYAN}bdd_top{C_RESET} - Linux Kernel Bitfield & Subsystem Monitor  {C_DIM}[Sort: {self.sort_key.upper()} | q: Quit | c: CPU | m: RES | u: USS]{C_RESET}"
        sys_info = f"Load avg: {C_BOLD}{load}{C_RESET}  Uptime: {up_str}  Tasks: {len(procs)}"

        lines.append(title)
        lines.append(sys_info)
        lines.append("")

        # 2-column layout for CPUs
        half = (len(cpu_lines) + 1) // 2
        col1 = cpu_lines[:half]
        col2 = cpu_lines[half:]
        for i in range(half):
            c1 = col1[i] if i < len(col1) else ""
            c2 = col2[i] if i < len(col2) else ""
            lines.append(f"  {c1}    {c2}")

        mem_meter = make_meter(mem_pct, width=20)
        swap_meter = make_meter(swap_pct, width=20)
        lines.append(f"  {C_BOLD}Mem {C_RESET} {mem_meter} {format_bytes(mem_used)}/{format_bytes(mem_total)}   {C_BOLD}Swap{C_RESET} {swap_meter} {format_bytes(swap_used)}/{format_bytes(swap_total)}")
        lines.append("")

        # Process Table Header
        header = f"{C_INVERSE}{'PID':>7} {'USER':<10} {'PRI':>3} {'NI':>3} {'VIRT':>7} {'RES':>7} {'USS':>7} {'S':<2} {'%CPU':>5} {'TIME+':>8} {'COMMAND'}{C_RESET}"
        lines.append(header)

        # Table entries
        avail_rows = rows - len(lines) - 1
        for p in procs[:max(1, avail_rows)]:
            cpu_color = C_BOLD + C_GREEN if p["cpu_pct"] > 5.0 else ""
            line = f"{p['pid']:>7} {p['user']:<10} {p['pri']:>3} {p['nice']:>3} {format_bytes(p['vsize_kb']):>7} {format_bytes(p['rss_kb']):>7} {format_bytes(p['uss_kb']):>7} {p['state']:<2} {cpu_color}{p['cpu_pct']:>5.1f}{C_RESET} {p['time_str']:>8} {p['comm']}"
            lines.append(line[:cols])

        # Output to screen
        output = "\033[H" + "\n".join(lines) + "\033[J"
        sys.stdout.write(output)
        sys.stdout.flush()

    def run(self):
        if self.once:
            procs = self.scan_processes(time.time())
            time.sleep(0.3)
            procs = self.scan_processes(time.time())
            self.render(procs, time.time())
            print()
            return

        old_settings = termios.tcgetattr(sys.stdin)
        try:
            tty.setcbreak(sys.stdin.fileno())
            sys.stdout.write("\033[?25l\033[2J") # hide cursor, clear screen
            sys.stdout.flush()

            # Warm-up scan for delta computation
            self.scan_processes(time.time())
            time.sleep(0.4)

            while self.running:
                now = time.time()
                procs = self.scan_processes(now)
                self.render(procs, now)

                # Non-blocking input check
                rlist, _, _ = select.select([sys.stdin], [], [], 1.2)
                if rlist:
                    ch = sys.stdin.read(1)
                    if ch in ('q', 'Q', '\x03'): # q or Ctrl+C
                        break
                    elif ch in ('c', 'C'):
                        self.sort_key = "cpu"
                    elif ch in ('m', 'M'):
                        self.sort_key = "mem"
                    elif ch in ('u', 'U'):
                        self.sort_key = "uss"
                    elif ch in ('p', 'P'):
                        self.sort_key = "pid"
        finally:
            termios.tcsetattr(sys.stdin, termios.TCSADRAIN, old_settings)
            sys.stdout.write("\033[?25h\n") # show cursor
            sys.stdout.flush()

def main():
    parser = argparse.ArgumentParser(description="Real-Time Bitfield System & Process Monitor (bdd_top)")
    parser.add_argument("--once", action="store_true", help="Print a single snapshot and exit (non-interactive)")
    parser.add_argument("--sort", choices=["cpu", "mem", "uss", "pid"], default="cpu", help="Initial sort column")
    args = parser.parse_args()

    top = BddTop(once=args.once)
    top.sort_key = args.sort
    top.run()

if __name__ == "__main__":
    main()
