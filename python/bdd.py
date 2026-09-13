"""
Python ctypes wrapper for libbdd.so native shared library.
Provides high-performance bitstream unpacking, packing, bit reversal,
and AI float format conversions (FP16, BF16, FP8, FP4).
"""

import ctypes
import os

def _find_lib():
    pkg_dir = os.path.dirname(os.path.abspath(__file__))
    base_dir = os.path.dirname(pkg_dir)
    candidates = [
        # 1. Bundled in package (installed via wheel)
        os.path.join(pkg_dir, "libbdd.so"),
        os.path.join(pkg_dir, "libbdd.dylib"),
        os.path.join(pkg_dir, "bdd.dll"),
        # 2. Local cargo target build paths (development tree)
        os.path.join(base_dir, "target", "release", "libbdd.so"),
        os.path.join(base_dir, "target", "debug", "libbdd.so"),
        os.path.join(base_dir, "target", "release", "libbdd.dylib"),
        os.path.join(base_dir, "target", "release", "bdd.dll"),
        # 3. Standard Linux / Unix system locations
        "/usr/local/lib/libbdd.so",
        "/usr/lib/libbdd.so",
        "/usr/lib64/libbdd.so",
    ]
    for c in candidates:
        if os.path.exists(c):
            return c
    import ctypes.util
    found = ctypes.util.find_library("bdd")
    if found:
        return found
    return "libbdd.so"

class Bdd:
    def __init__(self, lib_path=None):
        if lib_path is None:
            lib_path = _find_lib()
        self.lib = ctypes.CDLL(lib_path)

        self.lib.bdd_reverse_bits_u64.argtypes = [ctypes.c_uint64, ctypes.c_size_t]
        self.lib.bdd_reverse_bits_u64.restype = ctypes.c_uint64

        if hasattr(self.lib, "bdd_decode_f16"):
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

            self.lib.bdd_decode_fp8_e5m2.argtypes = [ctypes.c_uint8]
            self.lib.bdd_decode_fp8_e5m2.restype = ctypes.c_double

            self.lib.bdd_encode_fp8_e5m2.argtypes = [ctypes.c_double]
            self.lib.bdd_encode_fp8_e5m2.restype = ctypes.c_uint8

            self.lib.bdd_decode_fp6_e3m2.argtypes = [ctypes.c_uint8]
            self.lib.bdd_decode_fp6_e3m2.restype = ctypes.c_double

            self.lib.bdd_encode_fp6_e3m2.argtypes = [ctypes.c_double]
            self.lib.bdd_encode_fp6_e3m2.restype = ctypes.c_uint8

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

        if hasattr(self.lib, "bdd_read_bits_u64"):
            self.lib.bdd_read_bits_u64.argtypes = [
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
                ctypes.c_size_t,
                ctypes.c_size_t,
                ctypes.POINTER(ctypes.c_uint64),
            ]
            self.lib.bdd_read_bits_u64.restype = ctypes.c_int

            self.lib.bdd_write_bits_u64.argtypes = [
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
                ctypes.c_size_t,
                ctypes.c_size_t,
                ctypes.c_uint64,
            ]
            self.lib.bdd_write_bits_u64.restype = ctypes.c_int

            self.lib.bdd_copy_bits.argtypes = [
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
                ctypes.c_size_t,
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
                ctypes.c_size_t,
                ctypes.c_size_t,
            ]
            self.lib.bdd_copy_bits.restype = ctypes.c_int

            self.lib.bdd_unpack_buffer.argtypes = [
                ctypes.c_char_p,
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
                ctypes.c_size_t,
                ctypes.POINTER(ctypes.c_uint64),
                ctypes.c_size_t,
            ]
            self.lib.bdd_unpack_buffer.restype = ctypes.c_int

            self.lib.bdd_pack_buffer.argtypes = [
                ctypes.c_char_p,
                ctypes.POINTER(ctypes.c_uint64),
                ctypes.c_size_t,
                ctypes.POINTER(ctypes.c_uint8),
                ctypes.c_size_t,
                ctypes.c_size_t,
            ]
            self.lib.bdd_pack_buffer.restype = ctypes.c_int

            self.lib.bdd_pattern_total_bits.argtypes = [ctypes.c_char_p]
            self.lib.bdd_pattern_total_bits.restype = ctypes.c_int

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

    def decode_fp8_e5m2(self, bits: int) -> float:
        return self.lib.bdd_decode_fp8_e5m2(bits)

    def encode_fp8_e5m2(self, val: float) -> int:
        return self.lib.bdd_encode_fp8_e5m2(val)

    def decode_fp6(self, bits: int) -> float:
        return self.lib.bdd_decode_fp6_e3m2(bits)

    def encode_fp6(self, val: float) -> int:
        return self.lib.bdd_encode_fp6_e3m2(val)

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

    def pattern_total_bits(self, pattern: str) -> int:
        bits = self.lib.bdd_pattern_total_bits(pattern.encode("utf-8"))
        if bits < 0:
            raise ValueError(f"invalid pattern '{pattern}' (code {bits})")
        return bits

    def read_bits(self, buf, bit_offset: int, bit_count: int) -> int:
        """Reads up to 64 bits from a buffer at an arbitrary non-8-bit-aligned offset."""
        if isinstance(buf, (bytes, bytearray)):
            c_buf = (ctypes.c_uint8 * len(buf)).from_buffer_copy(buf) if isinstance(buf, bytes) else (ctypes.c_uint8 * len(buf)).from_buffer(buf)
            buf_len = len(buf)
        else:
            b = bytes(buf)
            c_buf = (ctypes.c_uint8 * len(b)).from_buffer_copy(b)
            buf_len = len(b)
        out_val = ctypes.c_uint64(0)
        res = self.lib.bdd_read_bits_u64(c_buf, buf_len, bit_offset, bit_count, ctypes.byref(out_val))
        if res < 0:
            raise ValueError(f"bdd_read_bits_u64 failed with error code {res}")
        return out_val.value

    def write_bits(self, buf: bytearray, bit_offset: int, bit_count: int, val: int):
        """Writes up to 64 bits into a mutable bytearray buffer at an arbitrary bit offset."""
        c_buf = (ctypes.c_uint8 * len(buf)).from_buffer(buf)
        res = self.lib.bdd_write_bits_u64(c_buf, len(buf), bit_offset, bit_count, val)
        if res < 0:
            raise ValueError(f"bdd_write_bits_u64 failed with error code {res}")

    def copy_bits(self, src, src_bit_offset: int, dst: bytearray, dst_bit_offset: int, bit_count: int):
        """Copies bit_count bits from src to dst across arbitrary non-8-bit boundaries."""
        src_bytes = bytes(src) if not isinstance(src, (bytes, bytearray)) else src
        c_src = (ctypes.c_uint8 * len(src_bytes)).from_buffer_copy(src_bytes) if isinstance(src_bytes, bytes) else (ctypes.c_uint8 * len(src_bytes)).from_buffer(src_bytes)
        c_dst = (ctypes.c_uint8 * len(dst)).from_buffer(dst)
        res = self.lib.bdd_copy_bits(c_src, len(src_bytes), src_bit_offset, c_dst, len(dst), dst_bit_offset, bit_count)
        if res < 0:
            raise ValueError(f"bdd_copy_bits failed with error code {res}")

    def unpack_buffer(self, pattern: str, src, bit_offset: int = 0, max_fields: int = 64):
        """Unpacks tuple fields directly from an in-memory buffer starting at an unaligned bit offset."""
        src_bytes = bytes(src) if not isinstance(src, (bytes, bytearray)) else src
        c_src = (ctypes.c_uint8 * len(src_bytes)).from_buffer_copy(src_bytes) if isinstance(src_bytes, bytes) else (ctypes.c_uint8 * len(src_bytes)).from_buffer(src_bytes)
        out_buf = (ctypes.c_uint64 * max_fields)()
        n = self.lib.bdd_unpack_buffer(pattern.encode("utf-8"), c_src, len(src_bytes), bit_offset, out_buf, max_fields)
        if n < 0:
            raise ValueError(f"bdd_unpack_buffer failed with error code {n}")
        return tuple(out_buf[i] for i in range(n))

    def pack_buffer(self, pattern: str, fields, dst: bytearray, bit_offset: int = 0) -> int:
        """Packs tuple fields directly into a mutable bytearray buffer starting at an unaligned bit offset."""
        num = len(fields)
        in_buf = (ctypes.c_uint64 * num)(*fields)
        c_dst = (ctypes.c_uint8 * len(dst)).from_buffer(dst)
        written = self.lib.bdd_pack_buffer(pattern.encode("utf-8"), in_buf, num, c_dst, len(dst), bit_offset)
        if written < 0:
            raise ValueError(f"bdd_pack_buffer failed with error code {written}")
        return written

    def reader(self, data, bit_offset: int = 0, bit_len=None):
        return BitStreamReader(data, bit_offset, bit_len, self)

    def writer(self):
        return BitStreamWriter(self)


_default_bdd = None

def get_bdd():
    global _default_bdd
    if _default_bdd is None:
        _default_bdd = Bdd()
    return _default_bdd

def read_bits(buf, bit_offset: int, bit_count: int) -> int:
    return get_bdd().read_bits(buf, bit_offset, bit_count)

def write_bits(buf: bytearray, bit_offset: int, bit_count: int, val: int):
    return get_bdd().write_bits(buf, bit_offset, bit_count, val)

def copy_bits(src, src_bit_offset: int, dst: bytearray, dst_bit_offset: int, bit_count: int):
    return get_bdd().copy_bits(src, src_bit_offset, dst, dst_bit_offset, bit_count)

def unpack_buffer(pattern: str, src, bit_offset: int = 0, max_fields: int = 64):
    return get_bdd().unpack_buffer(pattern, src, bit_offset, max_fields)

def pack_buffer(pattern: str, fields, dst: bytearray, bit_offset: int = 0) -> int:
    return get_bdd().pack_buffer(pattern, fields, dst, bit_offset)


class BitStreamReader:
    """
    Ergonomic reader for arbitrary-width and non-8-bit-aligned bitstreams in Python.
    """
    def __init__(self, data, bit_offset: int = 0, bit_len=None, bdd=None):
        self.bdd = bdd or get_bdd()
        self.data = bytes(data) if not isinstance(data, (bytes, bytearray)) else data
        self.bit_pos = bit_offset
        total_data_bits = len(self.data) * 8
        if bit_len is None:
            self.bit_len = total_data_bits
        else:
            if bit_offset + bit_len > total_data_bits:
                raise ValueError(
                    f"bit window [{bit_offset}..{bit_offset + bit_len}] exceeds buffer bit length {total_data_bits}"
                )
            self.bit_len = bit_offset + bit_len

    def remaining_bits(self) -> int:
        return max(0, self.bit_len - self.bit_pos)

    def is_empty(self) -> bool:
        return self.remaining_bits() == 0

    def pos(self) -> int:
        return self.bit_pos

    def total_bits(self) -> int:
        return self.bit_len

    def seek(self, bit_pos: int):
        if bit_pos > self.bit_len or bit_pos < 0:
            raise ValueError(f"seek position {bit_pos} out of bounds (limit {self.bit_len})")
        self.bit_pos = bit_pos

    def skip(self, bits: int):
        if bits > self.remaining_bits():
            raise ValueError(f"skip {bits} exceeds remaining {self.remaining_bits()} bits")
        self.bit_pos += bits

    def read(self, bits: int) -> int:
        if bits > self.remaining_bits():
            raise ValueError(f"read({bits}) exceeds remaining {self.remaining_bits()} bits")
        val = self.bdd.read_bits(self.data, self.bit_pos, bits)
        self.bit_pos += bits
        return val

    def read_tuple(self, pattern: str) -> tuple:
        total = self.bdd.pattern_total_bits(pattern)
        if total > self.remaining_bits():
            raise ValueError(f"pattern '{pattern}' requires {total} bits but only {self.remaining_bits()} remaining")
        fields = self.bdd.unpack_buffer(pattern, self.data, self.bit_pos)
        self.bit_pos += total
        return fields

    def iter_units(self, unit_bits: int, gap_bits: int = 0):
        while self.remaining_bits() >= unit_bits:
            yield self.read(unit_bits)
            if gap_bits > 0 and self.remaining_bits() >= gap_bits:
                self.skip(gap_bits)


class BitStreamWriter:
    """
    Ergonomic builder for non-8-bit-aligned bitstreams in Python.
    Accumulates arbitrary-width bitfields into packed bytes.
    """
    def __init__(self, bdd=None):
        self.bdd = bdd or get_bdd()
        self.buf = bytearray()
        self.total_bits = 0

    def write(self, val: int, bits: int):
        if bits == 0:
            return
        needed_bits = self.total_bits + bits
        needed_bytes = (needed_bits + 7) // 8
        if len(self.buf) < needed_bytes:
            self.buf.extend(b"\x00" * (needed_bytes - len(self.buf)))
        self.bdd.write_bits(self.buf, self.total_bits, bits, val)
        self.total_bits = needed_bits

    def write_tuple(self, pattern: str, fields):
        needed_bits = self.total_bits + self.bdd.pattern_total_bits(pattern)
        needed_bytes = (needed_bits + 7) // 8
        if len(self.buf) < needed_bytes:
            self.buf.extend(b"\x00" * (needed_bytes - len(self.buf)))
        written = self.bdd.pack_buffer(pattern, fields, self.buf, self.total_bits)
        self.total_bits += written

    def to_bytes(self) -> bytes:
        return bytes(self.buf)

    def finish(self):
        """Returns tuple of (bytes, exact_bit_length)."""
        return bytes(self.buf), self.total_bits

