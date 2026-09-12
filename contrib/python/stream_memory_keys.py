#!/usr/bin/env python3
"""stream_memory_keys.py: Stream system raw memory 256 bits at a time and probe cryptographic keys with bdd.

Demonstrates:
  1. Reading raw physical, kernel, or process virtual memory
  2. Streaming 256 bits (32 bytes) per unit into bdd
  3. Probing for high-entropy candidate cryptographic keys (AES-256, ChaCha20, Ed25519)
"""

import argparse
import os
import subprocess
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
BASE_DIR = os.path.abspath(os.path.join(SCRIPT_DIR, "../.."))
def find_bdd() -> str:
    env_bdd = os.environ.get("BDD_BIN")
    if env_bdd and os.path.isfile(env_bdd) and os.access(env_bdd, os.X_OK):
        return env_bdd
    for path in [
        os.path.join(BASE_DIR, "target/release/bdd"),
        os.path.join(BASE_DIR, "target/debug/bdd"),
        os.path.join(BASE_DIR, "bdd"),
    ]:
        if os.path.isfile(path) and os.access(path, os.X_OK):
            return path
    return "bdd"

def generate_demo_stream(num_units: int):
    """Generate in-memory demonstration stream containing real cryptographic keys."""
    # 1. Zero padding (~25% of units)
    p1 = max(8, num_units // 4)
    sys.stdout.buffer.write(b"\x00" * (p1 * 32))

    # 2. Known NIST AES-256 test key
    aes_key = bytes.fromhex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4")
    sys.stdout.buffer.write(aes_key)

    # 3. Structured text / config records
    sys.stdout.buffer.write(b"USER=admin\x00HOST=127.0.0.1\x00PORT=443\x00ENV=prod\x00" * 16)

    # 4. Known ChaCha20 256-bit test key
    chacha_key = bytes.fromhex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
    sys.stdout.buffer.write(chacha_key)

    # 5. Trailing zero padding
    written = (p1 * 32) + 32 + (16 * 44) + 32
    rem = max(0, (num_units * 32) - written)
    sys.stdout.buffer.write(b"\x00" * rem)
    sys.stdout.buffer.flush()

def stream_process_memory(pid: str, target_units: int, start_offset: int) -> bytes:
    """Read readable virtual memory mappings of process `pid`."""
    target_bytes = target_units * 32
    maps_file = f"/proc/{pid}/maps"
    mem_file = f"/proc/{pid}/mem"

    if not os.path.exists(maps_file):
        raise FileNotFoundError(f"Process maps {maps_file} does not exist")

    buf = bytearray()
    with open(mem_file, "rb", 0) as mem:
        with open(maps_file, "r") as maps:
            offset = start_offset
            for line in maps:
                parts = line.split()
                if len(parts) < 2:
                    continue
                perms = parts[1]
                if not perms.startswith("r"):
                    continue
                addr_range = parts[0].split("-")
                start_addr = int(addr_range[0], 16)
                end_addr = int(addr_range[1], 16)
                region_len = end_addr - start_addr

                if offset > 0:
                    if offset >= region_len:
                        offset -= region_len
                        continue
                    start_addr += offset
                    region_len -= offset
                    offset = 0

                chunk_len = min(region_len, target_bytes - len(buf))
                if chunk_len <= 0:
                    break

                try:
                    mem.seek(start_addr)
                    data = mem.read(chunk_len)
                    if data:
                        buf.extend(data)
                except (OSError, IOError):
                    pass

                if len(buf) >= target_bytes:
                    break
    return bytes(buf)

def main():
    parser = argparse.ArgumentParser(
        description="Stream system raw memory 256 bits at a time and probe cryptographic keys with bdd."
    )
    parser.add_argument(
        "-s", "--source",
        default="auto",
        help="Memory source: auto, kcore, devmem, self, pid <PID>, demo, or <FILE> (default: auto)"
    )
    parser.add_argument(
        "-p", "--pid",
        default=None,
        help="Target PID when source is 'pid'"
    )
    parser.add_argument(
        "-n", "--units",
        type=int,
        default=4096,
        help="Number of 256-bit units to stream (default: 4096 = 128 KiB)"
    )
    parser.add_argument(
        "-k", "--key-size",
        type=int,
        default=256,
        help="Key search window size in bits (default: 256)"
    )
    parser.add_argument(
        "-o", "--offset",
        type=int,
        default=0,
        help="Starting byte offset (default: 0)"
    )
    parser.add_argument(
        "-j", "--json",
        action="store_true",
        help="Emit raw JSON probe report from bdd"
    )

    args = parser.parse_args()
    bdd_bin = find_bdd()

    source = args.source
    if source == "auto":
        if os.geteuid() == 0 or subprocess.run(["sudo", "-n", "true"], capture_output=True).returncode == 0:
            source = "kcore"
        else:
            source = "self"

    bdd_cmd = [
        bdd_bin,
        "--input-unit=256",
        f"--probe-keys={args.key_size}",
    ]
    if args.json:
        bdd_cmd.append("--output-json")

    if not args.json:
        print("=" * 72)
        print(" Streaming System Raw Memory (256 bits/unit) into bdd Key Prober")
        print(f" Source:       {source}{f' (PID: {args.pid})' if args.pid else ''}")
        print(f" Unit Width:   256 bits (32 bytes per unit)")
        print(f" Units Count:  {args.units} units ({args.units * 32} bytes / {args.units * 256} bits)")
        print(f" Key Target:   {args.key_size} bits ({args.key_size // 8} bytes)")
        print(f" Memory Start: 0x{args.offset:X} ({args.offset} bytes)")
        print("=" * 72)

    # Prepare stream input
    if source == "demo":
        # Stream demo directly to bdd stdin
        proc = subprocess.Popen(bdd_cmd, stdin=subprocess.PIPE)
        # Generate demo data
        p1 = max(8, args.units // 4)
        data = bytearray(b"\x00" * (p1 * 32))
        aes_key = bytes.fromhex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4")
        data.extend(aes_key)
        data.extend(b"USER=admin\x00HOST=127.0.0.1\x00PORT=443\x00ENV=prod\x00" * 16)
        chacha_key = bytes.fromhex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")
        data.extend(chacha_key)
        rem = max(0, (args.units * 32) - len(data))
        data.extend(b"\x00" * rem)
        proc.communicate(input=bytes(data))
        sys.exit(proc.returncode)

    elif source in ("kcore", "devmem"):
        dev_file = "/proc/kcore" if source == "kcore" else "/dev/mem"
        dd_cmd = ["dd", f"if={dev_file}", "bs=32", f"skip={args.offset // 32}", f"count={args.units}", "status=none"]
        if os.geteuid() != 0:
            dd_cmd = ["sudo", "-n"] + dd_cmd
        dd_proc = subprocess.Popen(dd_cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        bdd_proc = subprocess.Popen(bdd_cmd, stdin=dd_proc.stdout)
        dd_proc.stdout.close()
        bdd_proc.communicate()
        sys.exit(bdd_proc.returncode)

    elif source in ("self", "pid"):
        target_pid = args.pid if (source == "pid" and args.pid) else str(os.getpid())
        data = stream_process_memory(target_pid, args.units, args.offset)
        proc = subprocess.Popen(bdd_cmd, stdin=subprocess.PIPE)
        proc.communicate(input=data)
        sys.exit(proc.returncode)

    elif os.path.isfile(source):
        with open(source, "rb") as f:
            if args.offset > 0:
                f.seek(args.offset)
            data = f.read(args.units * 32)
        proc = subprocess.Popen(bdd_cmd, stdin=subprocess.PIPE)
        proc.communicate(input=data)
        sys.exit(proc.returncode)
    else:
        sys.exit(f"Error: Unknown or inaccessible memory source '{source}'")

if __name__ == "__main__":
    main()
