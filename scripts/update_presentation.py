#!/usr/bin/env python3
"""
update_presentation.py: Updates docs/Presentation.odp with current README.md features:
- Rust v0.3.0 re-engineering & multi-gigabit throughput
- Raw units & offsets replacing pre-gap
- 64-bit architecture & exabyte-scale O(1) filesystem seeking
- Multipliers & modern AI float types (NVFP4, FP6, FP8, BF16)
- Ordered pipeline transformations (--round, --cut-maxint modes, --filter, --demux)
- Modern sinks (NDJSON, CSV, visual ANSI)
- New slides for Modern AI Weights, Multimedia Recipes, and Performance / C & Python APIs
- Complies strictly with OpenDocument zip format (uncompressed mimetype first)
"""

import os
import copy
import zipfile
import xml.etree.ElementTree as ET

ODP_PATH = "docs/Presentation.odp"

NAMESPACES = {
    'office': 'urn:oasis:names:tc:opendocument:xmlns:office:1.0',
    'style': 'urn:oasis:names:tc:opendocument:xmlns:style:1.0',
    'text': 'urn:oasis:names:tc:opendocument:xmlns:text:1.0',
    'table': 'urn:oasis:names:tc:opendocument:xmlns:table:1.0',
    'draw': 'urn:oasis:names:tc:opendocument:xmlns:drawing:1.0',
    'fo': 'urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0',
    'xlink': 'http://www.w3.org/1999/xlink',
    'dc': 'http://purl.org/dc/elements/1.1/',
    'meta': 'urn:oasis:names:tc:opendocument:xmlns:meta:1.0',
    'number': 'urn:oasis:names:tc:opendocument:xmlns:datastyle:1.0',
    'presentation': 'urn:oasis:names:tc:opendocument:xmlns:presentation:1.0',
    'svg': 'urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0',
    'chart': 'urn:oasis:names:tc:opendocument:xmlns:chart:1.0',
    'dr3d': 'urn:oasis:names:tc:opendocument:xmlns:dr3d:1.0',
    'math': 'http://www.w3.org/1998/Math/MathML',
    'form': 'urn:oasis:names:tc:opendocument:xmlns:form:1.0',
    'script': 'urn:oasis:names:tc:opendocument:xmlns:script:1.0',
    'ooo': 'http://openoffice.org/2004/office',
    'ooow': 'http://openoffice.org/2004/writer',
    'oooc': 'http://openoffice.org/2004/calc',
    'dom': 'http://www.w3.org/2001/xml-events',
    'xforms': 'http://www.w3.org/2002/xforms',
    'xsd': 'http://www.w3.org/2001/XMLSchema',
    'xsi': 'http://www.w3.org/2001/XMLSchema-instance',
    'smil': 'urn:oasis:names:tc:opendocument:xmlns:smil-compatible:1.0',
    'anim': 'urn:oasis:names:tc:opendocument:xmlns:animation:1.0',
}

for prefix, uri in NAMESPACES.items():
    ET.register_namespace(prefix, uri)

NS = {
    'draw': 'urn:oasis:names:tc:opendocument:xmlns:drawing:1.0',
    'presentation': 'urn:oasis:names:tc:opendocument:xmlns:presentation:1.0',
    'text': 'urn:oasis:names:tc:opendocument:xmlns:text:1.0',
    'xlink': 'http://www.w3.org/1999/xlink',
}

def qname(ns_prefix, tag):
    return f"{{{NAMESPACES[ns_prefix]}}}{tag}"

def set_slide_title(page, title_text):
    for frame in page.findall('.//draw:frame', NS):
        if frame.attrib.get(qname('presentation', 'class')) == 'title':
            tb = frame.find('draw:text-box', NS)
            if tb is not None:
                for child in list(tb):
                    tb.remove(child)
                p = ET.SubElement(tb, qname('text', 'p'))
                p.text = title_text
                return

def set_slide_bullets(page, bullets):
    for frame in page.findall('.//draw:frame', NS):
        cls = frame.attrib.get(qname('presentation', 'class'))
        if cls in ('outline', 'subtitle'):
            tb = frame.find('draw:text-box', NS)
            if tb is not None:
                for child in list(tb):
                    tb.remove(child)
                lst = ET.SubElement(tb, qname('text', 'list'), {qname('text', 'style-name'): 'L2'})
                for b in bullets:
                    li = ET.SubElement(lst, qname('text', 'list-item'))
                    p = ET.SubElement(li, qname('text', 'p'))
                    if isinstance(b, str):
                        lines = b.split('\n')
                        for idx, line in enumerate(lines):
                            if idx > 0:
                                ET.SubElement(p, qname('text', 'line-break'))
                            span = ET.SubElement(p, qname('text', 'span'))
                            span.text = line
                    elif isinstance(b, list):
                        for part_idx, part in enumerate(b):
                            text_val, is_code = part
                            span = ET.SubElement(p, qname('text', 'span'))
                            if is_code:
                                span.attrib[qname('text', 'style-name')] = 'T4'
                            span.text = text_val
                return

def create_bullet_slide(template_page, page_num, title, bullets):
    new_page = copy.deepcopy(template_page)
    new_page.attrib[qname('draw', 'name')] = f"page{page_num}"
    set_slide_title(new_page, title)
    set_slide_bullets(new_page, bullets)

    thumb = new_page.find('.//draw:page-thumbnail', NS)
    if thumb is not None:
        thumb.attrib[qname('draw', 'page-number')] = str(page_num)

    return new_page

def main():
    if not os.path.exists(ODP_PATH):
        print(f"Error: {ODP_PATH} not found")
        return

    temp_odp = ODP_PATH + ".tmp"
    with zipfile.ZipFile(ODP_PATH, 'r') as zin:
        content_xml = zin.read('content.xml')
        other_files = {name: zin.read(name) for name in zin.namelist() if name != 'content.xml'}

    root = ET.fromstring(content_xml)
    pages = root.findall('.//draw:page', NS)
    print(f"Loaded presentation with {len(pages)} original slides.")

    # -------------------------------------------------------------
    # 1. Update Slide 1 (Title slide)
    # -------------------------------------------------------------
    page1 = pages[0]
    for frame in page1.findall('.//draw:frame', NS):
        if frame.attrib.get(qname('presentation', 'class')) == 'title':
            tb = frame.find('draw:text-box', NS)
            if tb is not None:
                for child in list(tb):
                    tb.remove(child)
                p = ET.SubElement(tb, qname('text', 'p'))
                p.text = "bdd"
                ET.SubElement(p, qname('text', 'line-break'))
                span_open = ET.SubElement(p, qname('text', 'span'), {qname('text', 'style-name'): 'T1'})
                span_open.text = "("
                span_link = ET.SubElement(p, qname('text', 'span'), {qname('text', 'style-name'): 'T1'})
                a = ET.SubElement(span_link, qname('text', 'a'), {
                    qname('xlink', 'href'): 'https://github.com/e-t-u/bdd',
                    qname('xlink', 'type'): 'simple'
                })
                a.text = "https://github.com/e-t-u/bdd"
                span_close = ET.SubElement(p, qname('text', 'span'), {qname('text', 'style-name'): 'T1'})
                span_close.text = ")"

        elif frame.attrib.get(qname('presentation', 'class')) == 'subtitle':
            tb = frame.find('draw:text-box', NS)
            if tb is not None:
                for child in list(tb):
                    tb.remove(child)
                subtitles = [
                    "Bit Stream Manipulation Command & Native Library",
                    "For Unix/Linux/POSIX Command Line",
                    "Re-engineered in Modern Rust (v0.3.0)",
                    "Multi-Gigabit Throughput with 64-Bit Sub-Byte Precision",
                    "Open Source (GPL-3.0)",
                    "",
                    "By",
                    "Esa Turtiainen"
                ]
                for s in subtitles:
                    p = ET.SubElement(tb, qname('text', 'p'))
                    p.text = s

    # -------------------------------------------------------------
    # 2. Update Slide 2 (A Bit Stream)
    # -------------------------------------------------------------
    page2 = pages[1]
    for tb in page2.findall('.//draw:text-box', NS):
        paras = tb.findall('text:p', NS)
        for p in paras:
            if 'All measures in bits, not bytes' in ''.join(p.itertext()):
                for c in list(p):
                    p.remove(c)
                p.text = "All measures in bits, not bytes (Full 64-bit integer range)\nSupports scale suffixes: K, M, G, T, KiB, GiB, B\nSupports size math: 1920x1080*24, 1000000*24"

    # -------------------------------------------------------------
    # 3. Update Slide 3 (Raw Units & Containers instead of pre-gap)
    # -------------------------------------------------------------
    page3 = pages[2]
    set_slide_title(page3, "Raw Units & Containers")
    for frame in page3.findall('.//draw:frame', NS):
        tb = frame.find('draw:text-box', NS)
        if tb is not None:
            text_str = ''.join(tb.itertext())
            if '--input-pregap=1' in text_str:
                for child in list(tb):
                    tb.remove(child)
                p1 = ET.SubElement(tb, qname('text', 'p'))
                p1.text = "--input-raw-unit=8"
                p2 = ET.SubElement(tb, qname('text', 'p'))
                p2.text = "--input-offset=1"
                p3 = ET.SubElement(tb, qname('text', 'p'))
                p3.text = "--input-unit=2"
            elif '--input-pregap is more convenient' in text_str:
                for child in list(tb):
                    tb.remove(child)
                p1 = ET.SubElement(tb, qname('text', 'p'))
                p1.text = "Raw Unit is the repeating outer container, frame, or stride (e.g. 8, 16, 32 bits)"
                p2 = ET.SubElement(tb, qname('text', 'p'))
                p2.text = "--input-offset sets active unit offset inside container"
                p3 = ET.SubElement(tb, qname('text', 'p'))
                p3.text = "bdd derives skip & trailing gap (R - (O + U)) automatically"
                p4 = ET.SubElement(tb, qname('text', 'p'))
                p4.text = "Eliminates tedious manual arithmetic on structured bitstreams!"

    for p in page3.findall('.//text:p', NS):
        t = ''.join(p.itertext())
        if 'pre-gap' in t.lower() or 'pregap' in t.lower():
            for c in list(p):
                p.remove(c)
            p.text = None
            span = ET.SubElement(p, qname('text', 'span'), {qname('text', 'style-name'): 'T2'})
            span.text = "Offset"

    # -------------------------------------------------------------
    # 4. Update Slide 5 (Rearrange Bit Streams)
    # -------------------------------------------------------------
    page5 = pages[4]
    for p in page5.findall('.//text:p', NS):
        t = ''.join(p.itertext())
        if 'pregap' in t.lower():
            for c in list(p):
                p.remove(c)
            p.text = None
            span1 = ET.SubElement(p, qname('text', 'span'), {qname('text', 'style-name'): 'T4'})
            span1.text = "bdd --input-skip-bits=7 --input-unit=1 --input-gap=7 --output-unit=1"
            ET.SubElement(p, qname('text', 'line-break'))
            span2 = ET.SubElement(p, qname('text', 'span'), {qname('text', 'style-name'): 'T3'})
            span2.text = "(or with raw units: bdd --input-raw-unit=8 --input-offset=7 --input-unit=1 --output-unit=1)"

    # -------------------------------------------------------------
    # 5. Update Slide 6 (“Fake” Bit Streams -> Modern Sinks)
    # -------------------------------------------------------------
    page6 = pages[5]
    for frame in page6.findall('.//draw:frame', NS):
        tb = frame.find('draw:text-box', NS)
        if tb is not None:
            text_str = ''.join(tb.itertext())
            if 'Print unit as bits' in text_str:
                p1 = ET.SubElement(tb, qname('text', 'p'))
                p1.text = "Print unit as JSON (--output-json)"
                p2 = ET.SubElement(tb, qname('text', 'p'))
                p2.text = "  NDJSON record per tuple (for jq, Python, DBs)"
                p3 = ET.SubElement(tb, qname('text', 'p'))
                p3.text = "Print unit as CSV (--output-csv, --csv-header)"
                p4 = ET.SubElement(tb, qname('text', 'p'))
                p4.text = "Colorized visual dump (--output-visual)"
                p5 = ET.SubElement(tb, qname('text', 'p'))
                p5.text = "  Alternating ANSI colors for unaligned slices"

    # -------------------------------------------------------------
    # 6. Update Slide 7 (Default Unit Size & 64-Bit Seeking)
    # -------------------------------------------------------------
    page7 = pages[6]
    set_slide_title(page7, "Default Unit Size & 64-Bit Seeking")
    set_slide_bullets(page7, [
        "Rule to remember: bdd < a > b copies 8-bit bytes unchanged (byte-exact cat / dd)",
        "Default input unit: 8 bits | Default output unit: 8 bits",
        "Full 64-bit architecture: offsets & units up to 2.3 Exabytes (2^64-1 bits)",
        "Fast O(1) filesystem seek: jumps gigabytes in microseconds on disk files",
        "Automatic fallback to sequential streaming on pipes, FIFOs, and sockets (--no-seek)"
    ])

    # -------------------------------------------------------------
    # 7. Update Slide 10 (Pattern Specifiers Reference)
    # -------------------------------------------------------------
    page10 = pages[9]
    set_slide_bullets(page10, [
        "nU / nB — Unsigned integer / Byte (standard big-endian)",
        "nu / nb — Unsigned integer (reversed bit order)",
        "nS / ns — Two's complement signed integer (returns negative values)",
        "nM / nm — Sign-Split integer returning two fields: [sign, abs(val)]",
        "nx — Skip / discard n bits from input without placing in tuple",
        "Pattern Repetition Multipliers: 4*8B, 10*16H, 2*(4U4u)",
        "Byte-Level String Integrity: nC / nc preserves raw bytes without UTF-8 corruption"
    ])

    # -------------------------------------------------------------
    # 8. Update Slide 11 (“Fake” Tuple Streams -> Structured Sinks)
    # -------------------------------------------------------------
    page11 = pages[10]
    for frame in page11.findall('.//draw:frame', NS):
        tb = frame.find('draw:text-box', NS)
        if tb is not None:
            text_str = ''.join(tb.itertext())
            if '--output-tuples writes' in text_str:
                p1 = ET.SubElement(tb, qname('text', 'p'))
                p1.text = "--output-json writes newline-delimited JSON (NDJSON)"
                p2 = ET.SubElement(tb, qname('text', 'p'))
                p2.text = "--output-csv writes CSV (with optional --csv-header)"
                p3 = ET.SubElement(tb, qname('text', 'p'))
                p3.text = "--output-visual renders colorized ANSI field slices"

    # -------------------------------------------------------------
    # 9. Update Slide 13 (Floating Point & AI Tensor Formats)
    # -------------------------------------------------------------
    page13 = pages[12]
    set_slide_title(page13, "Floating Point & AI Tensor Formats")
    set_slide_bullets(page13, [
        "IEEE 754 Floats: 32F/32f (single), 64D/64d (double), 16H/16h (half FP16)",
        "Google Brain Bfloat16: 16Y / 16y (16 bits)",
        "OCP FP8 (E4M3FN): 8E / 8e (NVIDIA Hopper H100, 8 bits)",
        "OCP FP8 (E5M2): 8Q / 8q (NVIDIA Ada Lovelace, 8 bits)",
        "OCP Microscaling FP6: 6E / 6e (E3M2, 6 bits)",
        "NVIDIA Blackwell NVFP4: 4E / 4e (E2M1, 4 bits)",
        "Direct CLI float roundtrips: bdd --input-pattern=4E4E --output-json < nvfp4.bin"
    ])

    # -------------------------------------------------------------
    # 10. Update Slide 18 (Pipeline Diagram)
    # -------------------------------------------------------------
    page18 = pages[17]
    for frame in page18.findall('.//draw:frame', NS):
        tb = frame.find('draw:text-box', NS)
        if tb is not None:
            text_str = ''.join(tb.itertext())
            if '--remove-right' in text_str:
                for c in list(tb):
                    tb.remove(c)
                lines = [
                    "Rearrange Fields,",
                    "--shift-left, --shift-right",
                    "--round, --cut-maxint",
                    "--xor, --and, --or, --not",
                    "--add, --sub, --mul, --div",
                    "--abs, --sign, --filter, --demux"
                ]
                for l in lines:
                    p = ET.SubElement(tb, qname('text', 'p'))
                    p.text = l
            elif '”Fake” Bit Outputs:' in text_str:
                for p in tb.findall('text:p', NS):
                    if 'Integers,Hex' in ''.join(p.itertext()):
                        for c in list(p):
                            p.remove(c)
                        p.text = "JSON, CSV, Visual ANSI, Integers, Hex, Bits, Demux"

    # -------------------------------------------------------------
    # 11. Update Slide 19 (Ordered Pipeline Transformations)
    # -------------------------------------------------------------
    page19 = pages[18]
    set_slide_title(page19, "Ordered Pipeline Transformations")
    set_slide_bullets(page19, [
        "Manipulators execute in exact command-line order specified:",
        "Arithmetic: --add, --sub, --mul, --div, --mod, --abs, --sign",
        "Bitwise: --shift-left, --shift-right, --xor, --and, --or, --not",
        "Bounds & Rounding: --round, --cut-maxint=F,MAX,MODE\n(modes: saturate, wrap, drop, trunc, floor, ceil, round, round_ties_even)",
        "Predicate Filtering: --filter=\"FIELD,OP,VALUE\" (==, !=, <, <=, >, >=)",
        "Channel Demuxing: --demux 0:audio.raw --demux 1:video.raw (or --demux-files)"
    ])

    # -------------------------------------------------------------
    # 12. Append 3 New Modern Slides
    # -------------------------------------------------------------
    template_slide = pages[6]
    parent = None
    for p in root.iter():
        if list(p) and template_slide in list(p):
            parent = p
            break

    if parent is None:
        print("Could not find parent for slides.")
        return

    # Slide 23: Modern AI Quantized Weight Slicing
    slide23 = create_bullet_slide(
        template_slide, 23,
        "Modern AI Quantized Weight Slicing",
        [
            "Native Sub-Byte & Microscaling AI Formats:\n  NVIDIA Blackwell 4-bit NVFP4 (4E4E), OCP 6-bit FP6 (4*6E), FP8 (8E/8Q), BF16 (16Y)",
            "Instant O(1) Seeking into Large Weight Files:\n  Directly seek into HuggingFace Safetensors and GGUF files in microseconds without PyTorch/CUDA",
            "Weight Inspection & Decoding:\n  bdd --input-file=model.safetensors --input-skip-bits=${OFFSET}B --input-pattern=8E --output-json",
            "Sparsity & Outlier Detection:\n  bdd --input-pattern=8E --abs=0 --filter=\"0,>,3.5\" --output-json < tensor.bin"
        ]
    )
    parent.append(slide23)

    # Slide 24: Real-World Multimedia Slicing Recipes
    slide24 = create_bullet_slide(
        template_slide, 24,
        "Real-World Multimedia Slicing Recipes",
        [
            "MPEG Audio (MP3) Frame Headers:\n  Unaligned 32-bit header unpacked with single command: 11U2U2U1U4U2U1U1U2U2U1U1U2U",
            "MPEG-TS 188-byte Packet Containers:\n  Slices 13-bit PID: bdd --input-raw-unit=1504 --input-offset=11 --input-unit=13 --output-unit=13",
            "RIFF WAV Stereo Audio & Demuxing:\n  Splits interleaved 16-bit stereo PCM into left & right mono channels: --demux-files=l.raw,r.raw",
            "Studio 24-bit PCM Audio:\n  Reads 24S signed audio and downsamples to 16-bit: --remove-right=0,8 --output-unit=16",
            "JPEG SOF0 Marker:\n  Extracts 4-bit horizontal & vertical chroma subsampling factors (4U4U)"
        ]
    )
    parent.append(slide24)

    # Slide 25: Performance & Programmatic Interfaces
    slide25 = create_bullet_slide(
        template_slide, 25,
        "Performance & Programmatic Interfaces",
        [
            "Multi-Gigabit Modern Rust Engine:\n  • Hardware 64-bit bit reversal: 16 Gbps (direct CPU register acceleration)\n  • Bignum 1024-bit packing/unpacking: 5.27 Gbps\n  • Synthetic linear bit streaming: 200+ Mbps",
            "Native C-ABI Shared Library (libbdd.so & include/bdd.h):\n  bdd_unpack_u64(\"4U4U\", unit, fields, 2);\n  bdd_decode_fp8_e4m3(bits); bdd_decode_fp4_e2m1(bits);",
            "Zero-Dependency Python Wrapper (python/bdd.py):\n  from bdd import Bdd; b = Bdd(); b.decode_fp4(0x03)",
            "Extensive Examples Suite:\n  Complete runnable Shell, Python, and C programs in contrib/"
        ]
    )
    parent.append(slide25)

    print(f"Updated presentation to {len(root.findall('.//draw:page', NS))} slides.")

    # Save to ODP archive
    new_content_xml = ET.tostring(root, encoding='utf-8')

    # ODF standard: mimetype must be first entry, uncompressed
    mimetype_data = other_files.pop('mimetype', b'application/vnd.oasis.opendocument.presentation')

    with zipfile.ZipFile(temp_odp, 'w') as zout:
        zout.writestr('mimetype', mimetype_data, compress_type=zipfile.ZIP_STORED)
        zout.writestr('content.xml', new_content_xml, compress_type=zipfile.ZIP_DEFLATED)
        for name, data in other_files.items():
            zout.writestr(name, data, compress_type=zipfile.ZIP_DEFLATED)

    os.replace(temp_odp, ODP_PATH)
    print(f"Successfully wrote updated presentation to {ODP_PATH}!")

if __name__ == "__main__":
    main()
