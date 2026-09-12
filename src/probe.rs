//! Binary prober and layout analyzer for bdd.
//!
//! Inspects arbitrary streams and files to compute Shannon entropy, byte class distributions,
//! autocorrelation periodicities (repeating record strides), and ASCII string runs.

use crate::error::BddError;
use serde::Serialize;
use std::io::Read;

const MAX_PROBE_SAMPLE: usize = 1_048_576; // Sample up to 1 MB

#[derive(Debug, Clone, Serialize)]
pub struct ProbeReport {
    pub target: String,
    pub sample_bytes: usize,
    pub sample_bits: usize,
    pub entropy: f64,
    pub entropy_diagnosis: String,
    pub byte_distribution: ByteDistribution,
    pub detected_strides: Vec<DetectedStride>,
    pub string_runs: Vec<StringRun>,
    pub general_diagnosis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ByteDistribution {
    pub null_bytes: usize,
    pub null_percent: f64,
    pub printable_ascii: usize,
    pub ascii_percent: f64,
    pub high_bytes: usize,
    pub high_percent: f64,
    pub top_bytes: Vec<ByteFrequency>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ByteFrequency {
    pub byte_val: u8,
    pub hex: String,
    pub count: usize,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectedStride {
    pub stride_bytes: usize,
    pub stride_bits: usize,
    pub match_ratio: f64,
    pub confidence: String,
    pub likely_format: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct StringRun {
    pub offset: usize,
    pub length: usize,
    pub text: String,
}

pub fn probe_buffer(buf: &[u8], target_name: &str) -> ProbeReport {
    let n = buf.len();
    let sample_bits = n * 8;

    if n == 0 {
        return ProbeReport {
            target: target_name.to_string(),
            sample_bytes: 0,
            sample_bits: 0,
            entropy: 0.0,
            entropy_diagnosis: "Empty stream".to_string(),
            byte_distribution: ByteDistribution {
                null_bytes: 0,
                null_percent: 0.0,
                printable_ascii: 0,
                ascii_percent: 0.0,
                high_bytes: 0,
                high_percent: 0.0,
                top_bytes: Vec::new(),
            },
            detected_strides: Vec::new(),
            string_runs: Vec::new(),
            general_diagnosis: "Empty file or stream with 0 bytes.".to_string(),
        };
    }

    // 1. Compute frequency table
    let mut freq = [0usize; 256];
    let mut null_count = 0;
    let mut ascii_count = 0;
    let mut high_count = 0;

    for &b in buf {
        freq[b as usize] += 1;
        if b == 0 {
            null_count += 1;
        }
        if (0x20..=0x7E).contains(&b) || b == b'\t' || b == b'\r' || b == b'\n' {
            ascii_count += 1;
        }
        if b >= 0x80 {
            high_count += 1;
        }
    }

    // 2. Compute Shannon Entropy: H = -sum(p * log2(p))
    let mut entropy = 0.0f64;
    for &count in &freq {
        if count > 0 {
            let p = count as f64 / n as f64;
            entropy -= p * p.log2();
        }
    }

    let entropy_diagnosis = if entropy > 7.85 {
        "High (7.85-8.00): Likely compressed, encrypted, or packed floating-point weights"
            .to_string()
    } else if entropy > 6.5 {
        "Medium-High (6.50-7.85): Compiled machine code, uncompressed media, or packed structs"
            .to_string()
    } else if entropy > 4.0 {
        "Medium (4.00-6.50): Structured data, mixed binary headers and text payloads".to_string()
    } else if entropy > 1.5 {
        "Low (1.50-4.00): Plain ASCII text, sparse matrices, or structured tables".to_string()
    } else {
        "Very Low (<1.50): Nearly uniform or zero-padded stream".to_string()
    };

    // 3. Top frequent bytes
    let mut byte_pairs: Vec<(u8, usize)> = freq
        .iter()
        .enumerate()
        .map(|(b, &c)| (b as u8, c))
        .collect();
    byte_pairs.sort_by_key(|b| std::cmp::Reverse(b.1));
    let top_bytes: Vec<ByteFrequency> = byte_pairs
        .iter()
        .take(5)
        .filter(|(_, c)| *c > 0)
        .map(|&(b, count)| ByteFrequency {
            byte_val: b,
            hex: format!("0x{:02X}", b),
            count,
            percent: (count as f64 / n as f64) * 100.0,
        })
        .collect();

    // 4. Stride autocorrelation
    let mut strides = Vec::new();
    let max_stride = 512.min(n / 2);
    let baseline_p = 1.0 / 256.0;

    for s in 1..=max_stride {
        let cmp_len = n - s;
        let mut matches = 0;
        for i in 0..cmp_len {
            if buf[i] == buf[i + s] {
                matches += 1;
            }
        }
        let ratio = matches as f64 / cmp_len as f64;
        if ratio > baseline_p * 2.5 && ratio > 0.02 {
            let likely_format = match s {
                188 => "MPEG Transport Stream (188-byte packet stride)",
                204 => "MPEG-TS with Reed-Solomon parity (204-byte stride)",
                3 => "24-bit audio PCM or RGB24 pixel stride",
                4 => "32-bit audio PCM, RGBA pixel, or 32-bit word stride",
                2 => "16-bit audio PCM, RGB565, or 16-bit integer stride",
                6 => "OCP FP6 6-byte block stride",
                8 => "64-bit double / pointer word stride",
                16 => "128-bit SIMD block / UUID record stride",
                32 => "256-bit AVX register / SHA-256 block stride",
                44 => "RIFF/WAVE header chunk stride",
                54 => "BMP image header stride",
                64 => "64-byte CPU cache-line alignment",
                128 => "128-byte block record stride",
                512 => "512-byte sector block stride",
                _ => "Periodic structured record boundary",
            };
            let confidence = if ratio > 0.3 {
                "Very High"
            } else if ratio > 0.1 {
                "High"
            } else {
                "Moderate"
            };
            strides.push(DetectedStride {
                stride_bytes: s,
                stride_bits: s * 8,
                match_ratio: ratio,
                confidence: confidence.to_string(),
                likely_format,
            });
        }
    }
    // Sort strides by match ratio descending and keep top 5
    strides.sort_by(|a, b| {
        b.match_ratio
            .partial_cmp(&a.match_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    strides.truncate(5);

    // 5. Scan ASCII string runs (length >= 4)
    let mut string_runs = Vec::new();
    let mut current_run = Vec::new();
    let mut run_start = 0;

    for (i, &b) in buf.iter().enumerate() {
        if (0x20..=0x7E).contains(&b) {
            if current_run.is_empty() {
                run_start = i;
            }
            current_run.push(b);
        } else {
            if current_run.len() >= 4 {
                if let Ok(s) = String::from_utf8(current_run.clone()) {
                    string_runs.push(StringRun {
                        offset: run_start,
                        length: current_run.len(),
                        text: s,
                    });
                }
            }
            current_run.clear();
        }
    }
    if current_run.len() >= 4 {
        if let Ok(s) = String::from_utf8(current_run.clone()) {
            string_runs.push(StringRun {
                offset: run_start,
                length: current_run.len(),
                text: s,
            });
        }
    }
    string_runs.truncate(10);

    // 6. Overall diagnosis
    let general_diagnosis = if (null_count as f64 / n as f64) > 0.70 {
        "Sparse binary with heavy zero padding (suggests unallocated chunks or alignment offsets)."
            .to_string()
    } else if (ascii_count as f64 / n as f64) > 0.85 {
        "Plain text or ASCII structured payload.".to_string()
    } else if !strides.is_empty() && strides[0].stride_bytes == 188 {
        "MPEG Transport Stream binary (strong 188-byte periodic sync).".to_string()
    } else if !strides.is_empty() {
        format!(
            "Structured binary stream with periodic record stride of {} bytes ({} bits).",
            strides[0].stride_bytes, strides[0].stride_bits
        )
    } else if entropy > 7.8 {
        "High-entropy opaque payload (compressed, encrypted, or densely packed sub-byte floats)."
            .to_string()
    } else {
        "General binary payload with mixed data structures.".to_string()
    };

    ProbeReport {
        target: target_name.to_string(),
        sample_bytes: n,
        sample_bits,
        entropy,
        entropy_diagnosis,
        byte_distribution: ByteDistribution {
            null_bytes: null_count,
            null_percent: (null_count as f64 / n as f64) * 100.0,
            printable_ascii: ascii_count,
            ascii_percent: (ascii_count as f64 / n as f64) * 100.0,
            high_bytes: high_count,
            high_percent: (high_count as f64 / n as f64) * 100.0,
            top_bytes,
        },
        detected_strides: strides,
        string_runs,
        general_diagnosis,
    }
}

pub fn probe_reader<R: Read>(reader: &mut R, target_name: &str) -> Result<ProbeReport, BddError> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 65536];
    while buf.len() < MAX_PROBE_SAMPLE {
        let to_read = (MAX_PROBE_SAMPLE - buf.len()).min(chunk.len());
        match reader.read(&mut chunk[..to_read]) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) => return Err(BddError::from(e)),
        }
    }
    Ok(probe_buffer(&buf, target_name))
}

pub fn format_probe_text(report: &ProbeReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("Target:              {}\n", report.target));
    out.push_str(&format!(
        "Sample Size:         {} bytes ({} bits)\n",
        report.sample_bytes, report.sample_bits
    ));
    out.push_str(&format!(
        "Shannon Entropy:     {:.4} / 8.0000 bits/byte\n",
        report.entropy
    ));
    out.push_str(&format!(
        "Entropy Level:       {}\n",
        report.entropy_diagnosis
    ));
    out.push_str(&format!(
        "General Diagnosis:   {}\n\n",
        report.general_diagnosis
    ));

    out.push_str("Byte Class Distribution:\n");
    out.push_str(&format!(
        "  • Null Bytes (0x00):   {:>8}  ({:5.2}%)\n",
        report.byte_distribution.null_bytes, report.byte_distribution.null_percent
    ));
    out.push_str(&format!(
        "  • Printable ASCII:     {:>8}  ({:5.2}%)\n",
        report.byte_distribution.printable_ascii, report.byte_distribution.ascii_percent
    ));
    out.push_str(&format!(
        "  • High Bytes (>=0x80): {:>8}  ({:5.2}%)\n",
        report.byte_distribution.high_bytes, report.byte_distribution.high_percent
    ));

    if !report.byte_distribution.top_bytes.is_empty() {
        out.push_str("\nTop Frequent Byte Values:\n");
        for b in &report.byte_distribution.top_bytes {
            out.push_str(&format!(
                "  • {} ({:3}): {:>8} ({:5.2}%)\n",
                b.hex, b.byte_val, b.count, b.percent
            ));
        }
    }

    if !report.detected_strides.is_empty() {
        out.push_str("\nDetected Periodic Strides (Autocorrelation):\n");
        for s in &report.detected_strides {
            out.push_str(&format!(
                "  • {:>4} bytes ({:>5} bits) [Match: {:5.2}%, Conf: {:<9}]: {}\n",
                s.stride_bytes,
                s.stride_bits,
                s.match_ratio * 100.0,
                s.confidence,
                s.likely_format
            ));
        }
    }

    if !report.string_runs.is_empty() {
        out.push_str("\nIdentified ASCII String Runs:\n");
        for sr in &report.string_runs {
            out.push_str(&format!(
                "  • Offset 0x{:04X} (len {:2}): \"{}\"\n",
                sr.offset, sr.length, sr.text
            ));
        }
    }

    out
}

pub fn format_probe_json(report: &ProbeReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}
