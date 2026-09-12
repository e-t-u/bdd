//! Built-in binary format presets and schemas for bdd.
//!
//! Provides immediate unaligned bit patterns, unit sizing, endianness, and field names
//! for real-world multimedia containers, AI weight quantization formats, and network headers.

#[derive(Debug, Clone, Copy)]
pub struct Preset {
    pub name: &'static str,
    pub description: &'static str,
    pub pattern: &'static str,
    pub unit_bits: usize,
    pub little_endian: bool,
    pub default_count: Option<u64>,
}

pub static PRESETS: &[Preset] = &[
    Preset {
        name: "mp3-header",
        description: "MPEG Audio Frame Header (32-bit: sync, version, layer, bitrate, sampling rate, channels)",
        pattern: "sync:11u,version:2u,layer:2u,crc:1u,bitrate:4u,samplerate:2u,padding:1u,private:1u,channel:2u,mode_ext:2u,copyright:1u,original:1u,emphasis:2u",
        unit_bits: 32,
        little_endian: false,
        default_count: None,
    },
    Preset {
        name: "mpeg-ts",
        description: "MPEG Transport Stream 4-byte packet header (sync 0x47, TEI, PUSI, priority, 13-bit PID, counter)",
        pattern: "sync:8u,tei:1u,pusi:1u,priority:1u,pid:13u,scrambling:2u,adapt_ctrl:2u,counter:4u",
        unit_bits: 32,
        little_endian: false,
        default_count: None,
    },
    Preset {
        name: "wav-header",
        description: "RIFF/WAVE uncompressed audio 44-byte standard header",
        pattern: "riff:32C,file_size:32U,wave:32C,fmt:32C,fmt_size:32U,audio_fmt:16U,channels:16U,sample_rate:32U,byte_rate:32U,block_align:16U,bits_per_sample:16U,data:32C,data_size:32U",
        unit_bits: 352,
        little_endian: true,
        default_count: Some(1),
    },
    Preset {
        name: "jpeg-sof0",
        description: "JPEG SOF0 Baseline Frame Header (sample precision, image height, image width, components)",
        pattern: "precision:8u,height:16u,width:16u,components:8u",
        unit_bits: 48,
        little_endian: false,
        default_count: Some(1),
    },
    Preset {
        name: "h264-nal",
        description: "H.264 / AVC Network Abstraction Layer (NAL) 1-byte header",
        pattern: "forbidden_zero:1u,nal_ref_idc:2u,nal_unit_type:5u",
        unit_bits: 8,
        little_endian: false,
        default_count: None,
    },
    Preset {
        name: "nvfp4",
        description: "NVIDIA Blackwell / OCP FP4 (E2M1) packed sub-byte float pair (two 4-bit floats per byte)",
        pattern: "w0:4E,w1:4E",
        unit_bits: 8,
        little_endian: false,
        default_count: None,
    },
    Preset {
        name: "fp6-e3m2",
        description: "OCP Microscaling FP6 (E3M2) 4 unaligned 6-bit floats across 24 bits (3 bytes)",
        pattern: "w0:6E,w1:6E,w2:6E,w3:6E",
        unit_bits: 24,
        little_endian: false,
        default_count: None,
    },
    Preset {
        name: "fp8-e4m3",
        description: "OCP FP8 (E4M3FN) 8-bit floating-point weight",
        pattern: "w0:8E",
        unit_bits: 8,
        little_endian: false,
        default_count: None,
    },
    Preset {
        name: "fp8-e5m2",
        description: "OCP FP8 (E5M2) 8-bit floating-point weight",
        pattern: "w0:8Q",
        unit_bits: 8,
        little_endian: false,
        default_count: None,
    },
    Preset {
        name: "bf16",
        description: "Brain Floating Point 16-bit (Bfloat16) weight",
        pattern: "w0:16Y",
        unit_bits: 16,
        little_endian: false,
        default_count: None,
    },
    Preset {
        name: "fp16",
        description: "IEEE 754 Half-Precision 16-bit float",
        pattern: "w0:16H",
        unit_bits: 16,
        little_endian: false,
        default_count: None,
    },
    Preset {
        name: "ipv4-header",
        description: "IPv4 20-byte base packet header (version, IHL, DSCP, length, identification, flags, TTL, protocol, IPs)",
        pattern: "version:4U,ihl:4U,dscp:6U,ecn:2U,total_length:16U,id:16U,flags:3U,frag_offset:13U,ttl:8U,protocol:8U,checksum:16U,src_ip:32U,dst_ip:32U",
        unit_bits: 160,
        little_endian: false,
        default_count: Some(1),
    },
    Preset {
        name: "udp-header",
        description: "UDP 8-byte datagram header (source port, destination port, length, checksum)",
        pattern: "src_port:16U,dst_port:16U,length:16U,checksum:16U",
        unit_bits: 64,
        little_endian: false,
        default_count: Some(1),
    },
    Preset {
        name: "tcp-header",
        description: "TCP 20-byte base segment header (ports, sequence, ack, flags, window, checksum, urgent)",
        pattern: "src_port:16U,dst_port:16U,seq_num:32U,ack_num:32U,data_offset:4U,reserved:3U,ns:1B,cwr:1B,ece:1B,urg:1B,ack:1B,psh:1B,rst:1B,syn:1B,fin:1B,window_size:16U,checksum:16U,urg_ptr:16U",
        unit_bits: 160,
        little_endian: false,
        default_count: Some(1),
    },
    Preset {
        name: "riscv-r-type",
        description: "RISC-V 32-bit R-type instruction fields (funct7, rs2, rs1, funct3, rd, opcode)",
        pattern: "funct7:7u,rs2:5u,rs1:5u,funct3:3u,rd:5u,opcode:7u",
        unit_bits: 32,
        little_endian: false,
        default_count: None,
    },
];

/// Find a preset by name (case-insensitive, allows hyphens and underscores).
pub fn find_preset(name: &str) -> Option<&'static Preset> {
    let normalized = name.to_lowercase().replace('_', "-");
    PRESETS.iter().find(|p| p.name == normalized)
}

/// Returns all available presets.
pub fn all_presets() -> &'static [Preset] {
    PRESETS
}

/// Formats the list of available presets as an aligned human-readable table.
pub fn format_presets_table() -> String {
    let mut out = String::new();
    out.push_str("Available bdd Binary Format Presets:\n");
    out.push_str(&format!(
        "{:<14}  {:<8}  {:<50}  {}\n",
        "NAME", "UNIT", "DESCRIPTION", "PATTERN"
    ));
    out.push_str(&format!(
        "{:-<14}  {:-<8}  {:-<50}  {:-<30}\n",
        "", "", "", ""
    ));
    for p in PRESETS {
        let unit_str = format!("{}b ({}B)", p.unit_bits, p.unit_bits / 8);
        out.push_str(&format!(
            "{:<14}  {:<8}  {:<50}  {}\n",
            p.name, unit_str, p.description, p.pattern
        ));
    }
    out
}
