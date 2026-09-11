#!/usr/bin/env python3
"""
Generate realistic sample binary files for bdd examples:
- sample.mp3: MPEG-1 Layer III audio frame (128 kbps, 44.1 kHz, Joint Stereo)
- sample.ts: MPEG-2 Transport Stream (188-byte packets with PIDs 0, 256, 257)
- sample.wav: RIFF WAVE 16-bit stereo PCM audio (44.1 kHz)
- sample_24bit.raw: Raw 24-bit signed audio PCM samples
- sample.mp4: ISO-BMFF MP4 file with ftyp, moov, and mdat containing H.264 NAL units
- sample.jpg: JPEG JFIF image with SOF0 baseline DCT (640x480, 4:2:0 subsampling)
- sample.safetensors: Safetensors file containing FP8, BF16, and FP16 tensors
- sample_nvfp4.bin: Packed 4-bit NVIDIA Blackwell (NVFP4 E2M1) weights
- sample_fp6.bin: Packed 6-bit OCP (FP6 E3M2) weights (24-bit words)
"""

import os
import struct
import json
import math

def main():
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    data_dir = os.path.join(base_dir, "data")
    os.makedirs(data_dir, exist_ok=True)

    # -------------------------------------------------------------
    # 1. MP3 Frame Header & Frame (MPEG-1 Layer III, 128 kbps, 44.1kHz, Joint Stereo)
    # Header: 11-bit sync (0x7FF) | 2-bit ver (3=MPEG1) | 2-bit layer (1=L3) | 1-bit prot (1=no CRC)
    #         4-bit bitrate (9=128k) | 2-bit freq (0=44.1k) | 1-bit pad (0) | 1-bit priv (0)
    #         2-bit mode (1=Joint Stereo) | 2-bit ext (2) | 1-bit cprt (0) | 1-bit orig (1) | 2-bit emph (0)
    # Header hex: 0xFF 0xFB 0x90 0x64
    # Frame length: 144 * 128000 / 44100 = 417 bytes.
    # -------------------------------------------------------------
    mp3_header = bytes([0xFF, 0xFB, 0x90, 0x64])
    # Frame 1: 417 bytes total
    mp3_frame1 = mp3_header + bytes([(i * 7) % 256 for i in range(417 - 4)])
    # Frame 2: 160 kbps (bitrate index 10 = 0xA0)
    mp3_header2 = bytes([0xFF, 0xFB, 0xA0, 0x64])
    frame2_len = int(144 * 160000 / 44100)
    mp3_frame2 = mp3_header2 + bytes([(i * 13) % 256 for i in range(frame2_len - 4)])
    mp3_path = os.path.join(data_dir, "sample.mp3")
    with open(mp3_path, "wb") as f:
        f.write(mp3_frame1 + mp3_frame2)
    print(f"Generated {mp3_path} ({len(mp3_frame1) + len(mp3_frame2)} bytes, 2 frames)")

    # -------------------------------------------------------------
    # 2. MPEG-TS (188-byte packets)
    # Packet header: 4 bytes:
    #   sync (0x47)
    #   TEI(0), PUSI(1), Priority(0), PID (13 bits)
    #   Scrambling(0), Adaptation(1 = payload only), ContinuityCounter (4 bits)
    # -------------------------------------------------------------
    ts_packets = bytearray()
    # Packet 1: PID = 0 (PAT)
    # Header: 0x47, 0x40, 0x00, 0x10
    pkt1 = bytearray([0x47, 0x40, 0x00, 0x10]) + bytes([0x00] * 184)
    # Packet 2: PID = 256 (0x0100 - Video)
    # 13-bit PID 256 -> upper 5 bits: 0x01, lower 8 bits: 0x00
    # Header: 0x47, 0x41, 0x00, 0x11
    pkt2 = bytearray([0x47, 0x41, 0x00, 0x11]) + bytes([0xAA] * 184)
    # Packet 3: PID = 257 (0x0101 - Audio)
    # Header: 0x47, 0x41, 0x01, 0x12
    pkt3 = bytearray([0x47, 0x41, 0x01, 0x12]) + bytes([0xBB] * 184)
    # Packet 4: PID = 256 (Video)
    pkt4 = bytearray([0x47, 0x01, 0x00, 0x13]) + bytes([0xCC] * 184)

    ts_packets.extend(pkt1)
    ts_packets.extend(pkt2)
    ts_packets.extend(pkt3)
    ts_packets.extend(pkt4)

    ts_path = os.path.join(data_dir, "sample.ts")
    with open(ts_path, "wb") as f:
        f.write(ts_packets)
    print(f"Generated {ts_path} ({len(ts_packets)} bytes, 4 packets)")

    # -------------------------------------------------------------
    # 3. WAV (RIFF WAVE 16-bit Stereo PCM, 44.1 kHz, 100 samples)
    # -------------------------------------------------------------
    num_samples = 100
    sample_rate = 44100
    num_channels = 2
    bytes_per_sample = 2
    block_align = num_channels * bytes_per_sample
    byte_rate = sample_rate * block_align
    pcm_data = bytearray()
    for i in range(num_samples):
        # Left: 440 Hz tone, Right: 880 Hz tone
        l_val = int(16000 * math.sin(2 * math.pi * 440 * i / sample_rate))
        r_val = int(16000 * math.sin(2 * math.pi * 880 * i / sample_rate))
        pcm_data.extend(struct.pack("<hh", l_val, r_val))

    riff_header = struct.pack(
        "<4sI4s4sIHHIIHH4sI",
        b"RIFF",
        36 + len(pcm_data),
        b"WAVE",
        b"fmt ",
        16,             # Subchunk1Size
        1,              # AudioFormat (PCM)
        num_channels,   # 2 channels
        sample_rate,    # 44100
        byte_rate,      # 176400
        block_align,    # 4
        16,             # BitsPerSample
        b"data",
        len(pcm_data)
    )

    wav_path = os.path.join(data_dir, "sample.wav")
    with open(wav_path, "wb") as f:
        f.write(riff_header + pcm_data)
    print(f"Generated {wav_path} ({len(riff_header) + len(pcm_data)} bytes)")

    # -------------------------------------------------------------
    # 4. Raw 24-bit signed audio PCM samples
    # -------------------------------------------------------------
    raw_24bit_data = bytearray()
    sample_values_24 = [0, 1000000, 4000000, -1000000, -4000000, 8388607, -8388608]
    for val in sample_values_24:
        # 3 bytes, little-endian signed 24-bit
        b = val.to_bytes(3, byteorder="little", signed=True)
        raw_24bit_data.extend(b)
    raw24_path = os.path.join(data_dir, "sample_24bit.raw")
    with open(raw24_path, "wb") as f:
        f.write(raw_24bit_data)
    print(f"Generated {raw24_path} ({len(raw_24bit_data)} bytes, {len(sample_values_24)} samples)")

    # -------------------------------------------------------------
    # 5. MP4 / ISO-BMFF (ftyp, moov, and mdat containing H.264 NAL units)
    # -------------------------------------------------------------
    # ftyp box: 32 bytes
    ftyp_payload = b"isom\x00\x00\x02\x00isomiso2mp41"
    ftyp_box = struct.pack(">I4s", 8 + len(ftyp_payload), b"ftyp") + ftyp_payload

    # mdat box containing NAL units:
    # NAL Header: 1U2U5U
    # - NAL 1: 0x67 -> 0b01100111 -> 0, ref=3, type=7 (SPS)
    # - NAL 2: 0x68 -> 0b01101000 -> 0, ref=3, type=8 (PPS)
    # - NAL 3: 0x65 -> 0b01100101 -> 0, ref=3, type=5 (IDR Keyframe)
    # - NAL 4: 0x41 -> 0b01000001 -> 0, ref=2, type=1 (Non-IDR slice)
    nal_payload = (
        struct.pack(">I", 4) + bytes([0x67, 0x42, 0x00, 0x1E]) +  # SPS
        struct.pack(">I", 3) + bytes([0x68, 0xCE, 0x38]) +        # PPS
        struct.pack(">I", 5) + bytes([0x65, 0x88, 0x84, 0x21, 0x10]) + # IDR Keyframe
        struct.pack(">I", 4) + bytes([0x41, 0x9A, 0x12, 0x34])    # Slice
    )
    mdat_box = struct.pack(">I4s", 8 + len(nal_payload), b"mdat") + nal_payload

    # Minimal moov box
    moov_payload = b"\x00" * 16
    moov_box = struct.pack(">I4s", 8 + len(moov_payload), b"moov") + moov_payload

    mp4_path = os.path.join(data_dir, "sample.mp4")
    with open(mp4_path, "wb") as f:
        f.write(ftyp_box + mdat_box + moov_box)
    print(f"Generated {mp4_path} ({len(ftyp_box) + len(mdat_box) + len(moov_box)} bytes)")

    # -------------------------------------------------------------
    # 6. JPEG (JFIF with SOF0: 640x480, 4:2:0 subsampling)
    # -------------------------------------------------------------
    # SOF0 Marker (0xFFC0):
    # length: 17 bytes (0x0011)
    # precision: 8 bits
    # height: 480 (0x01E0)
    # width: 640 (0x0280)
    # components: 3
    # Component 1 (Y):  ID=1, Subsampling=0x22 (H=2, V=2), Quant table=0
    # Component 2 (Cb): ID=2, Subsampling=0x11 (H=1, V=1), Quant table=1
    # Component 3 (Cr): ID=3, Subsampling=0x11 (H=1, V=1), Quant table=1
    jpeg_data = bytearray()
    jpeg_data.extend([0xFF, 0xD8]) # SOI
    # APP0 JFIF
    app0_payload = b"JFIF\x00\x01\x01\x01\x00\x48\x00\x48\x00\x00"
    jpeg_data.extend([0xFF, 0xE0])
    jpeg_data.extend(struct.pack(">H", 2 + len(app0_payload)))
    jpeg_data.extend(app0_payload)

    # SOF0 (Baseline DCT)
    sof0_payload = struct.pack(
        ">BHHB" "BBB" "BBB" "BBB",
        8,          # precision
        480,        # height
        640,        # width
        3,          # components
        1, 0x22, 0, # Y: ID 1, 4U4U=2x2, Q 0
        2, 0x11, 1, # Cb: ID 2, 4U4U=1x1, Q 1
        3, 0x11, 1  # Cr: ID 3, 4U4U=1x1, Q 1
    )
    jpeg_data.extend([0xFF, 0xC0])
    jpeg_data.extend(struct.pack(">H", 2 + len(sof0_payload)))
    jpeg_data.extend(sof0_payload)
    jpeg_data.extend([0xFF, 0xD9]) # EOI

    jpg_path = os.path.join(data_dir, "sample.jpg")
    with open(jpg_path, "wb") as f:
        f.write(jpeg_data)
    print(f"Generated {jpg_path} ({len(jpeg_data)} bytes)")

    # -------------------------------------------------------------
    # 7. Safetensors file containing FP8, BF16, and FP16 weights
    # -------------------------------------------------------------
    # FP8 (E4M3): 4 values:
    #   0x38 = 1.0
    #   0x30 = 0.5
    #   0x40 = 2.0
    #   0xB8 = -1.0
    fp8_tensor = bytes([0x38, 0x30, 0x40, 0xB8])

    # BF16: 2 values:
    #   1.0 = 0x3F80
    #   2.0 = 0x4000
    bf16_tensor = struct.pack("<HH", 0x3F80, 0x4000)

    # FP16: 2 values:
    #   1.0 = 0x3C00
    #   -2.0 = 0xC000
    fp16_tensor = struct.pack("<HH", 0x3C00, 0xC000)

    tensors_buffer = fp8_tensor + bf16_tensor + fp16_tensor

    header_dict = {
        "model.layer0.fp8_weights": {
            "dtype": "F8_E4M3",
            "shape": [4],
            "data_offsets": [0, 4]
        },
        "model.layer0.bf16_weights": {
            "dtype": "BF16",
            "shape": [2],
            "data_offsets": [4, 8]
        },
        "model.layer0.f16_weights": {
            "dtype": "F16",
            "shape": [2],
            "data_offsets": [8, 12]
        }
    }
    header_json = json.dumps(header_dict, separators=(",", ":")).encode("utf-8")
    pad_len = (8 - (len(header_json) % 8)) % 8
    header_json += b" " * pad_len
    header_len_prefix = struct.pack("<Q", len(header_json))

    safetensors_path = os.path.join(data_dir, "sample.safetensors")
    with open(safetensors_path, "wb") as f:
        f.write(header_len_prefix + header_json + tensors_buffer)
    print(f"Generated {safetensors_path} ({8 + len(header_json) + len(tensors_buffer)} bytes, header={len(header_json)} bytes)")

    # -------------------------------------------------------------
    # 8. Raw NVIDIA Blackwell NVFP4 (E2M1) packed weights (4E4E)
    # -------------------------------------------------------------
    # Byte 0: [0.0, 0.5] -> 0x01
    # Byte 1: [1.0, 1.5] -> 0x23
    # Byte 2: [2.0, 3.0] -> 0x45
    # Byte 3: [4.0, 6.0] -> 0x67
    # Byte 4: [-0.0, -0.5] -> 0x89
    # Byte 5: [-1.0, -1.5] -> 0xAB
    # Byte 6: [-2.0, -3.0] -> 0xCD
    # Byte 7: [-4.0, -6.0] -> 0xEF
    nvfp4_bytes = bytes([0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF])
    nvfp4_path = os.path.join(data_dir, "sample_nvfp4.bin")
    with open(nvfp4_path, "wb") as f:
        f.write(nvfp4_bytes)
    print(f"Generated {nvfp4_path} ({len(nvfp4_bytes)} bytes, 16 NVFP4 weights)")

    # -------------------------------------------------------------
    # 9. Raw OCP FP6 (E3M2) packed weights (4x6-bit = 24-bit words)
    # -------------------------------------------------------------
    # Word 1: 0x10 (2.0), 0x08 (1.0), 0x04 (0.5), 0x30 (-2.0)
    word1 = (0x10 << 18) | (0x08 << 12) | (0x04 << 6) | 0x30
    bytes1 = struct.pack(">I", word1)[1:] # 3 bytes big-endian
    # Word 2: 0x0C (1.5), 0x14 (3.0), 0x02 (0.25), 0x00 (0.0)
    word2 = (0x0C << 18) | (0x14 << 12) | (0x02 << 6) | 0x00
    bytes2 = struct.pack(">I", word2)[1:]

    fp6_bytes = bytes1 + bytes2
    fp6_path = os.path.join(data_dir, "sample_fp6.bin")
    with open(fp6_path, "wb") as f:
        f.write(fp6_bytes)
    print(f"Generated {fp6_path} ({len(fp6_bytes)} bytes, 8 FP6 weights)")

    print("\nAll sample test files successfully generated in contrib/data/!")

if __name__ == "__main__":
    main()
