#!/usr/bin/env python3
"""
decode_media.py: Python demonstration of libbdd C-ABI bindings for multimedia decoding.
Demonstrates:
- MP3 frame header bitfield unpacking (11-bit sync, 2-bit ver, 4-bit bitrate...)
- MPEG-TS packet header unpacking (13-bit PID, continuity counter)
- JPEG SOF0 chroma subsampling nibble unpacking (4U4U)
- H.264 NAL unit header unpacking (1U2U5U)
"""

import os
import sys
import struct

# Ensure python/ directory is in sys.path
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
BASE_DIR = os.path.dirname(os.path.dirname(SCRIPT_DIR))
DATA_DIR = os.path.join(os.path.dirname(SCRIPT_DIR), "data")
sys.path.insert(0, os.path.join(BASE_DIR, "python"))

from bdd import Bdd

def decode_mp3(bdd: Bdd):
    mp3_path = os.path.join(DATA_DIR, "sample.mp3")
    print("=================================================================")
    print(" 1. MP3 Frame Header Decoding via libbdd")
    print("=================================================================")
    with open(mp3_path, "rb") as f:
        header_bytes = f.read(4)

    # 4 bytes = 32-bit big-endian integer unit
    unit = struct.unpack(">I", header_bytes)[0]
    pattern = "11U2U2U1U4U2U1U1U2U2U1U1U2U"
    fields = bdd.unpack(pattern, unit)

    sync, ver, layer, prot, br_idx, freq_idx, pad, priv, mode, ext, cprt, orig, emph = fields

    ver_str = ["MPEG-2.5", None, "MPEG-2", "MPEG-1"][ver]
    layer_str = ["Reserved", "Layer III (MP3)", "Layer II", "Layer I"][layer]
    kbps_table = [None, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320]
    kbps = kbps_table[br_idx]
    freq_str = ["44.1 kHz", "48.0 kHz", "32.0 kHz", "Reserved"][freq_idx]
    mode_str = ["Stereo", "Joint Stereo", "Dual Channel", "Mono"][mode]

    print(f"  Raw 32-bit Header: 0x{unit:08X}")
    print(f"  Syncword:          0x{sync:X} ({'Valid' if sync == 0x7FF else 'Invalid'})")
    print(f"  Format:            {ver_str} {layer_str}")
    print(f"  Bitrate:           {kbps} kbps")
    print(f"  Sample Rate:       {freq_str}")
    print(f"  Channel Mode:      {mode_str}")
    print(f"  Padding:           {bool(pad)}")
    print(f"  CRC Protected:     {not bool(prot)}")
    print(f"  Original:          {bool(orig)}")

def decode_mpeg_ts(bdd: Bdd):
    ts_path = os.path.join(DATA_DIR, "sample.ts")
    print("\n=================================================================")
    print(" 2. MPEG Transport Stream (MPEG-TS) Packet Headers via libbdd")
    print("=================================================================")
    pattern = "8U1U1U1U13U2U2U4U"

    with open(ts_path, "rb") as f:
        pkt_idx = 0
        while True:
            pkt = f.read(188)
            if not pkt or len(pkt) < 4:
                break
            header_u32 = struct.unpack(">I", pkt[:4])[0]
            sync, tei, pusi, prio, pid, scrm, adapt, counter = bdd.unpack(pattern, header_u32)

            desc = "PAT" if pid == 0 else ("Video (H.264)" if pid == 256 else ("Audio (AAC)" if pid == 257 else "Other"))
            print(f"  Packet #{pkt_idx}: Sync=0x{sync:02X} | PID={pid:4d} (0x{pid:04X}) [{desc:13s}] | PUSI={pusi} | Counter={counter}")
            pkt_idx += 1

def decode_jpeg(bdd: Bdd):
    jpg_path = os.path.join(DATA_DIR, "sample.jpg")
    print("\n=================================================================")
    print(" 3. JPEG SOF0 Chroma Subsampling Nibbles via libbdd")
    print("=================================================================")
    # SOF0 is at byte offset 20; component bytes are at offset 24 + 6 = byte 30
    with open(jpg_path, "rb") as f:
        f.seek(24)
        precision = f.read(1)[0]
        height, width, num_components = struct.unpack(">HHB", f.read(5))
        print(f"  Resolution: {width} x {height}, Precision: {precision}-bit, Components: {num_components}")

        comp_pattern = "8U4U4U8U" # ID (8U), H-factor (4U), V-factor (4U), QuantTable (8U)
        for i in range(num_components):
            raw = f.read(3)
            comp_u24 = (raw[0] << 16) | (raw[1] << 8) | raw[2]
            cid, h, v, q = bdd.unpack(comp_pattern, comp_u24)
            name = ["Y (Luma)", "Cb (Chroma Blue)", "Cr (Chroma Red)"][i]
            print(f"  Component {cid} [{name}]: Subsampling={h}x{v} ({'4:2:0' if h==2 and v==2 else '1x1'}), QuantTable={q}")

def decode_h264_nal(bdd: Bdd):
    mp4_path = os.path.join(DATA_DIR, "sample.mp4")
    print("\n=================================================================")
    print(" 4. H.264 NAL Unit Headers via libbdd")
    print("=================================================================")
    # mdat payload starts at offset 36 in our sample
    with open(mp4_path, "rb") as f:
        f.seek(36)
        nal_names = {1: "Non-IDR Slice", 5: "IDR Keyframe", 7: "SPS", 8: "PPS"}
        while f.tell() < 68:
            len_bytes = f.read(4)
            if not len_bytes or len(len_bytes) < 4:
                break
            nal_len = struct.unpack(">I", len_bytes)[0]
            header_byte = f.read(1)[0]
            # Unpack 1U2U5U
            zero, ref_idc, nal_type = bdd.unpack("1U2U5U", header_byte)
            name = nal_names.get(nal_type, "Other")
            print(f"  NAL Type {nal_type:2d} [{name:15s}] | Ref IDC: {ref_idc} | Forbidden Zero: {zero} | Length: {nal_len}B")
            f.seek(nal_len - 1, os.SEEK_CUR)

def main():
    bdd = Bdd()
    decode_mp3(bdd)
    decode_mpeg_ts(bdd)
    decode_jpeg(bdd)
    decode_h264_nal(bdd)
    print("\nAll multimedia Python examples executed successfully!")

if __name__ == "__main__":
    main()
