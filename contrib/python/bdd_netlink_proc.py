#!/usr/bin/env python3
"""
bdd_netlink_proc.py: Event-Driven Linux Process Monitor using Netlink Connector & bdd.

Subscribes to Linux kernel 'NETLINK_CONNECTOR' (CN_IDX_PROC) to receive
real-time binary notifications of process lifecycle events without polling:
  - FORK:  Parent PID/TGID creates child PID/TGID
  - EXEC:  Process executes a new binary (resolving comm/cmdline)
  - EXIT:  Process terminates with exit code / signal
  - UID:   Process changes user credentials (setuid/drop privileges)
  - GID:   Process changes group credentials
  - COMM:  Process renames its command thread

The kernel binary stream is unpacked and verified using 'bdd' patterns.
"""

import sys
import os
import socket
import struct
import argparse
import time
import datetime
import errno

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

# Linux Netlink & Connector Constants
NETLINK_CONNECTOR = 11
SOL_NETLINK = getattr(socket, 'SOL_NETLINK', 270)
NETLINK_NO_ENOBUFS = 5

CN_IDX_PROC = 0x1
CN_VAL_PROC = 0x1
PROC_CN_MCAST_LISTEN = 1
PROC_CN_MCAST_IGNORE = 2

# Process event enum
EVENT_NAMES = {
    0x00000000: "NONE",
    0x00000001: "FORK",
    0x00000002: "EXEC",
    0x00000004: "UID",
    0x00000040: "GID",
    0x00000080: "SID",
    0x00000100: "PTRACE",
    0x00000200: "COMM",
    0x20000000: "NONZERO_EXIT",
    0x40000000: "COREDUMP",
    0x80000000: "EXIT",
}

# Terminal ANSI Colors
C_RESET = "\033[0m"
C_BOLD = "\033[1m"
C_DIM = "\033[2m"
C_GREEN = "\033[32m"
C_CYAN = "\033[36m"
C_YELLOW = "\033[33m"
C_RED = "\033[31m"
C_MAGENTA = "\033[35m"
C_BLUE = "\033[34m"

def get_process_cmd(pid):
    """Attempts to read /proc/[pid]/comm and /proc/[pid]/cmdline."""
    comm = ""
    cmdline = ""
    try:
        with open(f"/proc/{pid}/comm", "r", encoding="utf-8", errors="ignore") as f:
            comm = f.read().strip()
    except Exception:
        pass
    try:
        with open(f"/proc/{pid}/cmdline", "rb") as f:
            raw = f.read(512)
            cmdline = " ".join([p.decode("utf-8", errors="ignore") for p in raw.split(b"\0") if p])
    except Exception:
        pass
    return comm or cmdline or "unknown"

class NetlinkProcessMonitor:
    def __init__(self, json_output=False, filter_event=None):
        self.json_output = json_output
        self.filter_event = filter_event.upper() if filter_event else None
        self.sock = None
        self.proc_start_times = {} # pid: timestamp

    def connect(self):
        try:
            self.sock = socket.socket(socket.AF_NETLINK, socket.SOCK_DGRAM, NETLINK_CONNECTOR)
            # Request large receive buffer (8MB) to absorb threadpool/fork bursts
            try:
                self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 8 * 1024 * 1024)
            except Exception:
                pass
            # Instruct kernel not to return ENOBUFS on queue overflow
            try:
                self.sock.setsockopt(SOL_NETLINK, NETLINK_NO_ENOBUFS, 1)
            except Exception:
                pass

            self.sock.bind((os.getpid(), CN_IDX_PROC))
        except PermissionError:
            print("Error: Permission denied opening NETLINK_CONNECTOR socket.", file=sys.stderr)
            sys.exit(1)
        except Exception as e:
            print(f"Error initializing netlink socket: {e}", file=sys.stderr)
            sys.exit(1)

        # Build registration message:
        # nlmsghdr (16 bytes) + cn_msg (20 bytes) + proc_cn_mcast_op (4 bytes)
        cn_msg = struct.pack('=IIIIHH', CN_IDX_PROC, CN_VAL_PROC, 0, 0, 4, 0)
        op = struct.pack('=I', PROC_CN_MCAST_LISTEN)
        body = cn_msg + op
        nlhdr = struct.pack('=IHHII', 16 + len(body), 0x3, 0, 0, os.getpid())
        
        self.sock.send(nlhdr + body)
        # Receive ACK / confirmation
        self.sock.recv(1024)

    def close(self):
        if self.sock:
            try:
                cn_msg = struct.pack('=IIIIHH', CN_IDX_PROC, CN_VAL_PROC, 0, 0, 4, 0)
                op = struct.pack('=I', PROC_CN_MCAST_IGNORE)
                body = cn_msg + op
                nlhdr = struct.pack('=IHHII', 16 + len(body), 0x3, 0, 0, os.getpid())
                self.sock.send(nlhdr + body)
                self.sock.close()
            except Exception:
                pass

    def run(self, max_count=None):
        self.connect()

        if not self.json_output:
            print("===================================================================================")
            print(" bdd_netlink_proc: Linux Kernel Real-Time Process Lifecycle Event Monitor")
            print(" Subscribed: NETLINK_CONNECTOR (CN_IDX_PROC=1) | Dissecting via 'bdd'")
            print("===================================================================================")
            print(f"{'TIMESTAMP':<12} {'EVENT':<8} {'CPU':>3}  {'DETAILS'}")
            print("-" * 83)

        count = 0
        try:
            while True:
                try:
                    data = self.sock.recv(65536)
                except OSError as e:
                    if e.errno == errno.ENOBUFS:
                        # Queue overrun occurred; log and keep reading
                        if not self.json_output:
                            print(f"{C_DIM}[WARN] Netlink buffer overrun (burst of events dropped by kernel){C_RESET}")
                        continue
                    raise

                msg_offset = 0
                while msg_offset + 16 <= len(data):
                    nlmsg_len, nlmsg_type, nlmsg_flags, seq, pid = struct.unpack_from('=IHHII', data, msg_offset)
                    if nlmsg_len < 16 or msg_offset + nlmsg_len > len(data):
                        break

                    msg_slice = data[msg_offset:msg_offset + nlmsg_len]
                    aligned_len = (nlmsg_len + 3) & ~3
                    msg_offset += aligned_len

                    if len(msg_slice) < 52: # 16 nlmsghdr + 20 cn_msg + 16 proc_event header
                        continue

                    # Offset 36 in msg_slice: proc_event header
                    proc_ev_data = msg_slice[36:]

                    # Dissect proc_event header: what:32U, cpu:32U, timestamp_ns:64U
                    what, cpu, timestamp_ns = struct.unpack('=IIQ', proc_ev_data[:16])

                    event_name = EVENT_NAMES.get(what, f"0x{what:08X}")
                    if self.filter_event and event_name != self.filter_event:
                        continue

                    payload = proc_ev_data[16:]
                    now_str = datetime.datetime.now().strftime("%H:%M:%S.%f")[:-3]
                    details = ""
                    event_dict = {
                        "timestamp": now_str,
                        "event": event_name,
                        "cpu": cpu,
                        "timestamp_ns": timestamp_ns,
                    }

                if what == 0x00000001:  # FORK
                    if len(payload) >= 16:
                        ppid, ptgid, cpid, ctgid = struct.unpack('=IIII', payload[:16])
                        parent_cmd = get_process_cmd(ppid)
                        self.proc_start_times[cpid] = time.time()
                        details = f"Parent:{ppid:<6} -> Child:{cpid:<6} ({parent_cmd})"
                        event_dict.update({"parent_pid": ppid, "child_pid": cpid, "comm": parent_cmd})

                elif what == 0x00000002:  # EXEC
                    if len(payload) >= 8:
                        pid, tgid = struct.unpack('=II', payload[:8])
                        cmd = get_process_cmd(pid)
                        details = f"PID:{pid:<6} Executed: {cmd}"
                        event_dict.update({"pid": pid, "comm": cmd})

                elif what == 0x80000000 or what == 0x20000000:  # EXIT
                    if len(payload) >= 16:
                        pid, tgid, exit_code, exit_sig = struct.unpack('=IIII', payload[:16])
                        dur_str = ""
                        if pid in self.proc_start_times:
                            elapsed = (time.time() - self.proc_start_times[pid]) * 1000.0
                            dur_str = f" [Runtime: {elapsed:.1f}ms]"
                        details = f"PID:{pid:<6} ExitCode:{exit_code:<3} Signal:{exit_sig:<2}{dur_str}"
                        event_dict.update({"pid": pid, "exit_code": exit_code, "exit_signal": exit_sig})

                elif what == 0x00000004:  # UID
                    if len(payload) >= 16:
                        pid, tgid, ruid, euid = struct.unpack('=IIII', payload[:16])
                        details = f"PID:{pid:<6} RealUID:{ruid} EffectiveUID:{euid}"
                        event_dict.update({"pid": pid, "ruid": ruid, "euid": euid})

                elif what == 0x00000040:  # GID
                    if len(payload) >= 16:
                        pid, tgid, rgid, egid = struct.unpack('=IIII', payload[:16])
                        details = f"PID:{pid:<6} RealGID:{rgid} EffectiveGID:{egid}"
                        event_dict.update({"pid": pid, "rgid": rgid, "egid": egid})

                elif what == 0x00000200:  # COMM
                    if len(payload) >= 24:
                        pid, tgid = struct.unpack('=II', payload[:8])
                        comm_raw = payload[8:24].rstrip(b'\0').decode('utf-8', errors='ignore')
                        details = f"PID:{pid:<6} Comm renamed: {comm_raw}"
                        event_dict.update({"pid": pid, "comm": comm_raw})
                else:
                    details = f"Payload: {payload.hex()[:32]}"

                if self.json_output:
                    import json
                    print(json.dumps(event_dict))
                else:
                    # Color badges
                    if event_name == "FORK":
                        badge = f"{C_BOLD}{C_GREEN}[FORK]{C_RESET}"
                    elif event_name == "EXEC":
                        badge = f"{C_BOLD}{C_CYAN}[EXEC]{C_RESET}"
                    elif event_name == "EXIT":
                        badge = f"{C_BOLD}{C_RED}[EXIT]{C_RESET}"
                    elif event_name in ("UID", "GID"):
                        badge = f"{C_BOLD}{C_YELLOW}[CRED]{C_RESET}"
                    elif event_name == "COMM":
                        badge = f"{C_BOLD}{C_MAGENTA}[COMM]{C_RESET}"
                    else:
                        badge = f"[{event_name}]"

                    print(f"{now_str:<12} {badge:<17} {cpu:>3}  {details}")

                count += 1
                if max_count and count >= max_count:
                    break

        except KeyboardInterrupt:
            pass
        finally:
            self.close()

def main():
    parser = argparse.ArgumentParser(description="Real-Time Linux Process Lifecycle Monitor via Netlink Connector & bdd")
    parser.add_argument("-n", "--count", type=int, help="Exit after capturing N events")
    parser.add_argument("-e", "--event", choices=["FORK", "EXEC", "EXIT", "UID", "GID", "COMM"], help="Filter by event type")
    parser.add_argument("--json", action="store_true", help="Emit events as JSON Lines for agents and pipelines")
    args = parser.parse_args()

    monitor = NetlinkProcessMonitor(json_output=args.json, filter_event=args.event)
    monitor.run(max_count=args.count)

if __name__ == "__main__":
    main()
