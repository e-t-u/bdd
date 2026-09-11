"""
Python ctypes wrapper for libbdd.so native shared library.
Provides high-performance bitstream unpacking, packing, bit reversal,
and AI float format conversions (FP16, BF16, FP8, FP4).
"""

import ctypes
import os

def _find_lib():
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    candidates = [
        os.path.join(base_dir, "target", "release", "libbdd.so"),
        os.path.join(base_dir, "target", "debug", "libbdd.so"),
        os.path.join(base_dir, "target", "release", "libbdd.dylib"),
        os.path.join(base_dir, "target", "release", "bdd.dll"),
    ]
    for c in candidates:
        if os.path.exists(c):
            return c
    return "libbdd.so"

class Bdd:
    def __init__(self, lib_path=None):
        if lib_path is None:
            lib_path = _find_lib()
        self.lib = ctypes.CDLL(lib_path)

        self.lib.bdd_reverse_bits_u64.argtypes = [ctypes.c_uint64, ctypes.c_size_t]
        self.lib.bdd_reverse_bits_u64.restype = ctypes.c_uint64

        self.lib.bdd_decode_f16.argtypes = [ctypes.c_uint16]
        self.lib.bdd_decode_f16.restype = ctypes.c_double

        self.lib.bdd_encode_f16.argtypes = [ctypes.c_double]
        self.lib.bdd_encode_f16.restype = ctypes.c_uint16

        self.lib.bdd_decode_bf16.argtypes = [ctypes.c_uint16]
        self.lib.bdd_decode_bf16.restype = ctypes.c_double

        self.lib.bdd_encode_bf16.argtypes = [ctypes.c_double]
        self.lib.bdd_encode_bf16.restype = ctypes.c_uint16

        self.lib.bdd_decode_fp8_e4m3.argtypes = [ctypes.c_uint8]
        self.lib.bdd_decode_fp8_e4m3.restype = ctypes.c_double

        self.lib.bdd_encode_fp8_e4m3.argtypes = [ctypes.c_double]
        self.lib.bdd_encode_fp8_e4m3.restype = ctypes.c_uint8

        self.lib.bdd_decode_fp4_e2m1.argtypes = [ctypes.c_uint8]
        self.lib.bdd_decode_fp4_e2m1.restype = ctypes.c_double

        self.lib.bdd_encode_fp4_e2m1.argtypes = [ctypes.c_double]
        self.lib.bdd_encode_fp4_e2m1.restype = ctypes.c_uint8

        self.lib.bdd_unpack_u64.argtypes = [
            ctypes.c_char_p,
            ctypes.c_uint64,
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_size_t,
        ]
        self.lib.bdd_unpack_u64.restype = ctypes.c_int

        self.lib.bdd_pack_u64.argtypes = [
            ctypes.c_char_p,
            ctypes.POINTER(ctypes.c_uint64),
            ctypes.c_size_t,
            ctypes.POINTER(ctypes.c_uint64),
        ]
        self.lib.bdd_pack_u64.restype = ctypes.c_int

    def reverse_bits(self, val: int, bits: int) -> int:
        return self.lib.bdd_reverse_bits_u64(val, bits)

    def decode_f16(self, bits: int) -> float:
        return self.lib.bdd_decode_f16(bits)

    def encode_f16(self, val: float) -> int:
        return self.lib.bdd_encode_f16(val)

    def decode_bf16(self, bits: int) -> float:
        return self.lib.bdd_decode_bf16(bits)

    def encode_bf16(self, val: float) -> int:
        return self.lib.bdd_encode_bf16(val)

    def decode_fp8(self, bits: int) -> float:
        return self.lib.bdd_decode_fp8_e4m3(bits)

    def encode_fp8(self, val: float) -> int:
        return self.lib.bdd_encode_fp8_e4m3(val)

    def decode_fp4(self, bits: int) -> float:
        return self.lib.bdd_decode_fp4_e2m1(bits)

    def encode_fp4(self, val: float) -> int:
        return self.lib.bdd_encode_fp4_e2m1(val)

    def unpack(self, pattern: str, unit: int, max_fields: int = 64):
        out_buf = (ctypes.c_uint64 * max_fields)()
        n = self.lib.bdd_unpack_u64(pattern.encode("utf-8"), unit, out_buf, max_fields)
        if n < 0:
            raise ValueError(f"bdd_unpack_u64 failed with error code {n}")
        return tuple(out_buf[i] for i in range(n))

    def pack(self, pattern: str, fields):
        num = len(fields)
        in_buf = (ctypes.c_uint64 * num)(*fields)
        out_val = ctypes.c_uint64(0)
        res = self.lib.bdd_pack_u64(pattern.encode("utf-8"), in_buf, num, ctypes.byref(out_val))
        if res < 0:
            raise ValueError(f"bdd_pack_u64 failed with error code {res}")
        return out_val.value
