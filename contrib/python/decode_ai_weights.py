#!/usr/bin/env python3
"""
decode_ai_weights.py: Python demonstration of AI model weight decoding via libbdd.
Demonstrates:
- Safetensors header reading and direct tensor buffer decoding (FP8 E4M3, BF16, FP16)
- NVIDIA Blackwell NVFP4 (E2M1) 4-bit weight decoding
- OCP Microscaling FP6 (E3M2) 6-bit weight decoding
- Weight sparsity, zero-counting, and outlier analysis
"""

import os
import sys
import json
import struct

# Ensure python/ directory is in sys.path
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
BASE_DIR = os.path.dirname(os.path.dirname(SCRIPT_DIR))
DATA_DIR = os.path.join(os.path.dirname(SCRIPT_DIR), "data")
sys.path.insert(0, os.path.join(BASE_DIR, "python"))

from bdd import Bdd

def decode_safetensors(bdd: Bdd):
    path = os.path.join(DATA_DIR, "sample.safetensors")
    print("=================================================================")
    print(" 1. Safetensors Weight Inspection via libbdd")
    print("=================================================================")

    with open(path, "rb") as f:
        header_len_bytes = f.read(8)
        header_len = struct.unpack("<Q", header_len_bytes)[0]
        header_json = json.loads(f.read(header_len).decode("utf-8"))
        data_base_offset = 8 + header_len

        print(f"  Safetensors Header Size: {header_len} bytes")
        print(f"  Tensors found: {len(header_json)}")

        for tensor_name, info in header_json.items():
            dtype = info["dtype"]
            shape = info["shape"]
            start, end = info["data_offsets"]
            length = end - start

            f.seek(data_base_offset + start)
            raw_data = f.read(length)

            decoded_values = []
            if dtype == "F8_E4M3":
                for b in raw_data:
                    decoded_values.append(bdd.decode_fp8(b))
            elif dtype == "BF16":
                for i in range(0, length, 2):
                    bits = struct.unpack("<H", raw_data[i:i+2])[0]
                    decoded_values.append(bdd.decode_bf16(bits))
            elif dtype == "F16":
                for i in range(0, length, 2):
                    bits = struct.unpack("<H", raw_data[i:i+2])[0]
                    decoded_values.append(bdd.decode_f16(bits))

            print(f"\n  • Tensor: {tensor_name}")
            print(f"    DType: {dtype}, Shape: {shape}, Buffer: [{start}..{end}] ({length} bytes)")
            print(f"    Decoded Weights: {[round(v, 4) for v in decoded_values]}")

def decode_nvfp4(bdd: Bdd):
    path = os.path.join(DATA_DIR, "sample_nvfp4.bin")
    print("\n=================================================================")
    print(" 2. NVIDIA Blackwell NVFP4 (E2M1) Sub-Byte Weight Decoding")
    print("=================================================================")
    with open(path, "rb") as f:
        data = f.read()

    print(f"  Input size: {len(data)} bytes ({len(data)*2} 4-bit weights)")
    weights = []
    for byte_idx, b in enumerate(data):
        # Unpack two 4-bit nibbles using bdd pattern 4U4U
        w0_bits, w1_bits = bdd.unpack("4U4U", b)
        val0 = bdd.decode_fp4(w0_bits)
        val1 = bdd.decode_fp4(w1_bits)
        weights.extend([val0, val1])
        if byte_idx < 4:
            print(f"  Byte #{byte_idx} (0x{b:02X}) -> Nibble0: {w0_bits:04b} ({val0:5.2f}), Nibble1: {w1_bits:04b} ({val1:5.2f})")

    zeros = sum(1 for w in weights if w == 0.0)
    print(f"  Decoded {len(weights)} weights | Min: {min(weights)} | Max: {max(weights)} | Zeros: {zeros}")

def decode_fp6(bdd: Bdd):
    path = os.path.join(DATA_DIR, "sample_fp6.bin")
    print("\n=================================================================")
    print(" 3. OCP Microscaling FP6 (E3M2) Unaligned 6-bit Float Decoding")
    print("=================================================================")
    with open(path, "rb") as f:
        data = f.read()

    # 6-bit floats: 4 weights = 24 bits = 3 bytes
    print(f"  Input size: {len(data)} bytes ({len(data)*8 // 6} 6-bit weights)")
    weights = []
    for chunk_idx in range(0, len(data), 3):
        chunk = data[chunk_idx:chunk_idx+3]
        if len(chunk) < 3:
            break
        u24 = (chunk[0] << 16) | (chunk[1] << 8) | chunk[2]
        # Unpack four 6-bit fields using bdd pattern 6U6U6U6U
        f0, f1, f2, f3 = bdd.unpack("6U6U6U6U", u24)
        vals = [bdd.decode_fp6(f) for f in (f0, f1, f2, f3)]
        weights.extend(vals)
        print(f"  Chunk #{chunk_idx//3} (24-bit: 0x{u24:06X}) -> FP6 values: {[round(v, 4) for v in vals]}")

    print(f"  Total FP6 weights decoded: {len(weights)} | Outlier (|w| >= 2.0): {[w for w in weights if abs(w) >= 2.0]}")

def main():
    bdd = Bdd()
    decode_safetensors(bdd)
    decode_nvfp4(bdd)
    decode_fp6(bdd)
    print("\nAll AI weight Python examples executed successfully!")

if __name__ == "__main__":
    main()
