//! Binary prober and layout analyzer for bdd.
//!
//! Inspects arbitrary streams and files to compute Shannon entropy, byte class distributions,
//! autocorrelation periodicities (repeating record strides), and ASCII string runs.

use crate::error::BddError;
use num_bigint::BigUint;
use num_traits::{One, ToPrimitive, Zero};
use serde::Serialize;
use std::collections::HashMap;
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

#[derive(Debug, Clone, Serialize)]
pub struct UnitProbeReport {
    pub target: String,
    pub unit_bits: usize,
    pub total_units: usize,
    pub total_bits: usize,
    pub total_bytes: usize,
    pub distinct_units: usize,
    pub min_value: String,
    pub max_value: String,
    pub null_units: usize,
    pub null_percent: f64,
    pub all_ones_units: usize,
    pub all_ones_percent: f64,
    pub unit_entropy: f64,
    pub max_unit_entropy: f64,
    pub normalized_entropy: f64,
    pub entropy_diagnosis: String,
    pub top_units: Vec<UnitFrequency>,
    pub detected_strides: Vec<DetectedUnitStride>,
    pub crypto_key_candidates: Vec<CryptoKeyCandidate>,
    pub general_diagnosis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnitFrequency {
    pub value_dec: String,
    pub value_hex: String,
    pub count: usize,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectedUnitStride {
    pub stride_units: usize,
    pub stride_bits: usize,
    pub match_ratio: f64,
    pub confidence: String,
    pub likely_format: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CryptoKeyCandidate {
    pub rank: usize,
    pub start_unit: usize,
    pub unit_count: usize,
    pub bit_offset: usize,
    pub bit_length: usize,
    pub byte_length: usize,
    pub entropy: f64,
    pub max_entropy: f64,
    pub normalized_entropy: f64,
    pub bit_balance_percent: f64,
    pub hex_payload: String,
    pub likely_algorithm: String,
    pub confidence: String,
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

pub fn units_to_bytes(units: &[BigUint], unit_bits: usize) -> Vec<u8> {
    if unit_bits == 8 {
        return units
            .iter()
            .map(|u| (u & BigUint::from(0xFFu32)).to_u8().unwrap_or(0))
            .collect();
    }
    let mut out = Vec::new();
    let mut buffer = BigUint::zero();
    let mut bits_in_buffer = 0usize;
    let mask = if unit_bits > 0 {
        (BigUint::one() << unit_bits) - 1u32
    } else {
        BigUint::zero()
    };

    for u in units {
        let val = u & &mask;
        buffer = (buffer << unit_bits) | val;
        bits_in_buffer += unit_bits;

        while bits_in_buffer >= 8 {
            let shift = bits_in_buffer - 8;
            let byte_val = (&buffer >> shift).to_u8().unwrap_or(0);
            out.push(byte_val);
            bits_in_buffer -= 8;
            let rem_mask = if bits_in_buffer > 0 {
                (BigUint::one() << bits_in_buffer) - 1u32
            } else {
                BigUint::zero()
            };
            buffer &= rem_mask;
        }
    }
    if bits_in_buffer > 0 {
        let byte_val = (buffer << (8 - bits_in_buffer)).to_u8().unwrap_or(0);
        out.push(byte_val);
    }
    out
}

pub fn parse_key_bits(s: &str) -> Result<usize, BddError> {
    let s_trimmed = s.trim();
    if s_trimmed.is_empty() {
        return Ok(256);
    }
    let lower = s_trimmed.to_lowercase();
    if let Some(rest) = lower
        .strip_suffix("bytes")
        .or_else(|| lower.strip_suffix("byte"))
    {
        let num_str = rest.trim_end_matches(['-', '_', ' ']);
        let bytes: usize = num_str.parse().map_err(|_| {
            BddError::CliError(format!("Invalid key size '{}': cannot parse byte count", s))
        })?;
        if bytes == 0 {
            return Err(BddError::CliError("Key size cannot be 0".to_string()));
        }
        return Ok(bytes * 8);
    }
    if let Some(rest) = s_trimmed.strip_suffix('B') {
        let num_str = rest.trim_end_matches(['-', '_', ' ']);
        let bytes: usize = num_str.parse().map_err(|_| {
            BddError::CliError(format!("Invalid key size '{}': cannot parse byte count", s))
        })?;
        if bytes == 0 {
            return Err(BddError::CliError("Key size cannot be 0".to_string()));
        }
        return Ok(bytes * 8);
    }
    if let Some(rest) = lower
        .strip_suffix("bits")
        .or_else(|| lower.strip_suffix("bit"))
        .or_else(|| lower.strip_suffix('b'))
    {
        let num_str = rest.trim_end_matches(['-', '_', ' ']);
        let bits: usize = num_str.parse().map_err(|_| {
            BddError::CliError(format!("Invalid key size '{}': cannot parse bit count", s))
        })?;
        if bits == 0 {
            return Err(BddError::CliError("Key size cannot be 0".to_string()));
        }
        return Ok(bits);
    }
    let bits: usize = s_trimmed.parse().map_err(|_| {
        BddError::CliError(format!(
            "Invalid key size '{}': expected bits (e.g. 256) or bytes (e.g. 32B)",
            s
        ))
    })?;
    if bits == 0 {
        return Err(BddError::CliError("Key size cannot be 0".to_string()));
    }
    Ok(bits)
}

pub fn find_crypto_keys(
    bytes: &[u8],
    unit_bits: usize,
    key_bits: usize,
) -> Vec<CryptoKeyCandidate> {
    let key_bytes = key_bits.div_ceil(8);
    if key_bytes == 0 || bytes.len() < key_bytes {
        return Vec::new();
    }

    let n = bytes.len();
    let w = key_bytes;
    let max_h = (w as f64).min(256.0).log2();

    let mut freq = [0usize; 256];
    let mut ones_count = 0usize;

    for &b in &bytes[..w] {
        freq[b as usize] += 1;
        ones_count += b.count_ones() as usize;
    }

    let calc_entropy = |f: &[usize; 256]| -> f64 {
        let mut ent = 0.0f64;
        let w_f = w as f64;
        for &cnt in f {
            if cnt > 0 {
                let p = cnt as f64 / w_f;
                ent -= p * p.log2();
            }
        }
        ent
    };

    let total_bits_in_window = w * 8;
    let mut raw_candidates: Vec<(usize, f64, f64, f64, usize)> = Vec::new();

    let ent0 = calc_entropy(&freq);
    let norm0 = if max_h > 0.0 { ent0 / max_h } else { 0.0 };
    let bal0 = (ones_count as f64 / total_bits_in_window as f64) * 100.0;
    raw_candidates.push((0, ent0, norm0, bal0, freq[0]));

    for i in 1..=(n - w) {
        let old_b = bytes[i - 1];
        let new_b = bytes[i + w - 1];
        freq[old_b as usize] -= 1;
        freq[new_b as usize] += 1;
        ones_count -= old_b.count_ones() as usize;
        ones_count += new_b.count_ones() as usize;

        let ent = calc_entropy(&freq);
        let norm = if max_h > 0.0 { ent / max_h } else { 0.0 };
        let bal = (ones_count as f64 / total_bits_in_window as f64) * 100.0;
        raw_candidates.push((i, ent, norm, bal, freq[0]));
    }

    raw_candidates.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.4.cmp(&b.4))
            .then_with(|| {
                let diff_a = (a.3 - 50.0).abs();
                let diff_b = (b.3 - 50.0).abs();
                diff_a
                    .partial_cmp(&diff_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let mut selected = Vec::new();
    for (offset, ent, norm, bal, _) in raw_candidates {
        let overlaps = selected
            .iter()
            .any(|(sel_offset, _, _, _): &(usize, f64, f64, f64)| offset.abs_diff(*sel_offset) < w);
        if !overlaps {
            selected.push((offset, ent, norm, bal));
            if selected.len() >= 5 {
                break;
            }
        }
    }

    let likely_algo = match key_bits {
        128 => "AES-128 / Poly1305 / MD5 key or IV (128-bit)",
        192 => "AES-192 / 3DES key or 192-bit nonce (192-bit)",
        256 => "AES-256 / ChaCha20 / Ed25519 / SHA-256 key (256-bit)",
        512 => "Ed512 / SHA-512 / HMAC-512 key (512-bit)",
        k => Box::leak(format!("{}-bit Cryptographic Key Candidate", k).into_boxed_str()),
    };

    let mut out = Vec::new();
    for (rank, &(offset, ent, norm, bal)) in selected.iter().enumerate() {
        let bit_offset = offset * 8;
        let start_unit = bit_offset.checked_div(unit_bits).unwrap_or(0);
        let unit_count = if unit_bits > 0 {
            key_bits.div_ceil(unit_bits)
        } else {
            0
        };
        let payload = &bytes[offset..offset + w];
        let hex_payload: String = payload.iter().map(|b| format!("{:02x}", b)).collect();

        let confidence = if norm >= 0.98 {
            "Very High"
        } else if norm >= 0.93 {
            "High"
        } else if norm >= 0.85 {
            "Moderate"
        } else {
            "Low"
        };

        out.push(CryptoKeyCandidate {
            rank: rank + 1,
            start_unit,
            unit_count,
            bit_offset,
            bit_length: key_bits,
            byte_length: w,
            entropy: ent,
            max_entropy: max_h,
            normalized_entropy: norm,
            bit_balance_percent: bal,
            hex_payload,
            likely_algorithm: likely_algo.to_string(),
            confidence: confidence.to_string(),
        });
    }

    out
}

pub fn probe_unit_stream(
    units: &[BigUint],
    unit_bits: usize,
    key_search_bits: Option<usize>,
    target_name: &str,
) -> UnitProbeReport {
    let n = units.len();
    let total_bits = n * unit_bits;
    let total_bytes = total_bits.div_ceil(8);

    if n == 0 {
        return UnitProbeReport {
            target: target_name.to_string(),
            unit_bits,
            total_units: 0,
            total_bits: 0,
            total_bytes: 0,
            distinct_units: 0,
            min_value: "0".to_string(),
            max_value: "0".to_string(),
            null_units: 0,
            null_percent: 0.0,
            all_ones_units: 0,
            all_ones_percent: 0.0,
            unit_entropy: 0.0,
            max_unit_entropy: 0.0,
            normalized_entropy: 0.0,
            entropy_diagnosis: "Empty unit stream".to_string(),
            top_units: Vec::new(),
            detected_strides: Vec::new(),
            crypto_key_candidates: Vec::new(),
            general_diagnosis: "Empty unit stream (0 units processed).".to_string(),
        };
    }

    let mut freq: HashMap<BigUint, usize> = HashMap::new();
    let mut null_count = 0usize;
    let mut all_ones_count = 0usize;
    let all_ones_val = if unit_bits > 0 {
        (BigUint::one() << unit_bits) - 1u32
    } else {
        BigUint::zero()
    };

    let mut min_val: Option<BigUint> = None;
    let mut max_val: Option<BigUint> = None;

    for u in units {
        *freq.entry(u.clone()).or_insert(0) += 1;
        if u.is_zero() {
            null_count += 1;
        }
        if *u == all_ones_val {
            all_ones_count += 1;
        }
        min_val = Some(match min_val {
            Some(curr) => curr.min(u.clone()),
            None => u.clone(),
        });
        max_val = Some(match max_val {
            Some(curr) => curr.max(u.clone()),
            None => u.clone(),
        });
    }

    let distinct_units = freq.len();

    let mut unit_entropy = 0.0f64;
    let n_f = n as f64;
    for &count in freq.values() {
        if count > 0 {
            let p = count as f64 / n_f;
            unit_entropy -= p * p.log2();
        }
    }

    let max_theoretical = if unit_bits < 64 {
        (1u64 << unit_bits) as f64
    } else {
        f64::MAX
    };
    let max_unit_entropy = n_f.min(max_theoretical).log2();
    let normalized_entropy = if unit_bits > 0 {
        (unit_entropy / (unit_bits as f64)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let entropy_diagnosis = if normalized_entropy > 0.98 {
        "Near-maximal (0.98-1.00): Cryptographic randomness, cipher payload, or dense compressed stream".to_string()
    } else if normalized_entropy > 0.85 {
        "High (0.85-0.98): Compressed data, packed floating-point numbers, or dense bitfields"
            .to_string()
    } else if normalized_entropy > 0.50 {
        "Medium (0.50-0.85): Structured records, machine instructions, or mixed text/binary"
            .to_string()
    } else if normalized_entropy > 0.20 {
        "Low (0.20-0.50): Sparse data, ASCII text, or structured protocol headers".to_string()
    } else {
        "Very Low (<0.20): Constant, repetitive, or zero-dominated stream".to_string()
    };

    let mut unit_pairs: Vec<(&BigUint, usize)> = freq.iter().map(|(u, &c)| (u, c)).collect();
    unit_pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let hex_width = unit_bits.div_ceil(4).max(2);
    let top_units: Vec<UnitFrequency> = unit_pairs
        .iter()
        .take(5)
        .map(|&(u, count)| {
            let hex_str = u.to_str_radix(16).to_uppercase();
            let padded_hex = format!("0x{:0>width$}", hex_str, width = hex_width);
            UnitFrequency {
                value_dec: u.to_str_radix(10),
                value_hex: padded_hex,
                count,
                percent: (count as f64 / n_f) * 100.0,
            }
        })
        .collect();

    let mut detected_strides = Vec::new();
    let max_stride = 256.min(n / 2);
    let baseline_p = 1.0 / (distinct_units as f64).max(1.0);

    for s in 1..=max_stride {
        let cmp_len = n - s;
        let mut matches = 0;
        for i in 0..cmp_len {
            if units[i] == units[i + s] {
                matches += 1;
            }
        }
        let ratio = matches as f64 / cmp_len as f64;
        if ratio > baseline_p * 2.0 && ratio > 0.02 {
            let likely_format = match s {
                1 => "Consecutive identical units (repetition run)",
                2 => "Stereo audio or 2-unit interleaved record stride",
                3 => "RGB or 3-unit tuple stride",
                4 => "RGBA pixel or 4-channel audio stride",
                6 => "6-unit record stride",
                8 => "8-unit record stride / 64-bit word stride",
                16 => "16-unit SIMD / 128-bit block stride",
                32 => "32-unit AVX / SHA-256 block stride",
                64 => "64-unit cacheline stride",
                188 if unit_bits == 8 => "MPEG Transport Stream (188-byte packet stride)",
                204 if unit_bits == 8 => "MPEG-TS with Reed-Solomon parity (204-byte stride)",
                _ => "Periodic structured record boundary",
            };
            let confidence = if ratio > 0.3 {
                "Very High"
            } else if ratio > 0.1 {
                "High"
            } else {
                "Moderate"
            };
            detected_strides.push(DetectedUnitStride {
                stride_units: s,
                stride_bits: s * unit_bits,
                match_ratio: ratio,
                confidence: confidence.to_string(),
                likely_format,
            });
        }
    }
    detected_strides.sort_by(|a, b| {
        b.match_ratio
            .partial_cmp(&a.match_ratio)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    detected_strides.truncate(5);

    let bytes = units_to_bytes(units, unit_bits);
    let key_bits = key_search_bits.unwrap_or(256);
    let crypto_key_candidates = find_crypto_keys(&bytes, unit_bits, key_bits);

    let general_diagnosis = if !crypto_key_candidates.is_empty()
        && (crypto_key_candidates[0].confidence == "Very High"
            || crypto_key_candidates[0].confidence == "High")
    {
        format!(
            "High-entropy unit stream with strong {} candidate at unit offset {} (bit {}, entropy {:.4}/{:.4}).",
            crypto_key_candidates[0].likely_algorithm,
            crypto_key_candidates[0].start_unit,
            crypto_key_candidates[0].bit_offset,
            crypto_key_candidates[0].entropy,
            crypto_key_candidates[0].max_entropy
        )
    } else if (null_count as f64 / n_f) > 0.70 {
        "Sparse unit stream with heavy zero-padding.".to_string()
    } else if !detected_strides.is_empty() {
        format!(
            "Structured unit stream with periodic record stride of {} units ({} bits).",
            detected_strides[0].stride_units, detected_strides[0].stride_bits
        )
    } else if normalized_entropy > 0.90 {
        "High-entropy opaque unit stream (compressed, encrypted, or packed float payload)."
            .to_string()
    } else {
        "Structured unit stream with mixed data structures.".to_string()
    };

    let min_val_str = min_val
        .map(|u| {
            let h = u.to_str_radix(16).to_uppercase();
            format!(
                "{} (0x{:0>width$})",
                u.to_str_radix(10),
                h,
                width = hex_width
            )
        })
        .unwrap_or_else(|| "0".to_string());

    let max_val_str = max_val
        .map(|u| {
            let h = u.to_str_radix(16).to_uppercase();
            format!(
                "{} (0x{:0>width$})",
                u.to_str_radix(10),
                h,
                width = hex_width
            )
        })
        .unwrap_or_else(|| "0".to_string());

    UnitProbeReport {
        target: target_name.to_string(),
        unit_bits,
        total_units: n,
        total_bits,
        total_bytes,
        distinct_units,
        min_value: min_val_str,
        max_value: max_val_str,
        null_units: null_count,
        null_percent: (null_count as f64 / n_f) * 100.0,
        all_ones_units: all_ones_count,
        all_ones_percent: (all_ones_count as f64 / n_f) * 100.0,
        unit_entropy,
        max_unit_entropy,
        normalized_entropy,
        entropy_diagnosis,
        top_units,
        detected_strides,
        crypto_key_candidates,
        general_diagnosis,
    }
}

pub fn format_unit_probe_text(report: &UnitProbeReport) -> String {
    let mut out = String::new();
    out.push_str(
        "================================================================================\n",
    );
    out.push_str("bdd Unit Stream Prober (Post-Input Stream Processing)\n");
    out.push_str(
        "================================================================================\n",
    );
    out.push_str(&format!("Target:              {}\n", report.target));
    out.push_str(&format!("Unit Width:          {} bits\n", report.unit_bits));
    out.push_str(&format!(
        "Sample Size:         {} units ({} bits / {} bytes)\n",
        report.total_units, report.total_bits, report.total_bytes
    ));
    out.push_str(&format!(
        "Shannon Entropy:     {:.4} / {:.4} bits/unit (Normalized: {:.4})\n",
        report.unit_entropy, report.max_unit_entropy, report.normalized_entropy
    ));
    out.push_str(&format!(
        "Entropy Level:       {}\n",
        report.entropy_diagnosis
    ));
    out.push_str(&format!(
        "General Diagnosis:   {}\n\n",
        report.general_diagnosis
    ));

    out.push_str("Unit Value Statistics:\n");
    out.push_str(&format!(
        "  • Distinct Values:     {:>8} / {}\n",
        report.distinct_units, report.total_units
    ));
    out.push_str(&format!("  • Minimum Value:       {}\n", report.min_value));
    out.push_str(&format!("  • Maximum Value:       {}\n", report.max_value));
    out.push_str(&format!(
        "  • Null Units (0x00):   {:>8}  ({:5.2}%)\n",
        report.null_units, report.null_percent
    ));
    out.push_str(&format!(
        "  • Max Units (All-1s):  {:>8}  ({:5.2}%)\n",
        report.all_ones_units, report.all_ones_percent
    ));

    if !report.top_units.is_empty() {
        out.push_str("\nTop Frequent Unit Values:\n");
        for u in &report.top_units {
            out.push_str(&format!(
                "  • {:>8} ({:>6}): {:>8} ({:5.2}%)\n",
                u.value_hex, u.value_dec, u.count, u.percent
            ));
        }
    }

    if !report.detected_strides.is_empty() {
        out.push_str("\nDetected Periodic Unit Strides (Autocorrelation):\n");
        for s in &report.detected_strides {
            out.push_str(&format!(
                "  • {:>4} units ({:>5} bits) [Match: {:5.2}%, Conf: {:<9}]: {}\n",
                s.stride_units,
                s.stride_bits,
                s.match_ratio * 100.0,
                s.confidence,
                s.likely_format
            ));
        }
    }

    if !report.crypto_key_candidates.is_empty() {
        out.push_str("\nPotential Maximum-Entropy Cryptographic Keys:\n");
        for c in &report.crypto_key_candidates {
            out.push_str(&format!(
                "  #{} [{}] Unit offset: {} (bit {}, byte 0x{:04X}), Len: {} bits ({} bytes)\n",
                c.rank,
                c.confidence,
                c.start_unit,
                c.bit_offset,
                c.bit_offset / 8,
                c.bit_length,
                c.byte_length
            ));
            out.push_str(&format!(
                "     Entropy:        {:.4} / {:.4} bits/byte (Normalized: {:.4})\n",
                c.entropy, c.max_entropy, c.normalized_entropy
            ));
            out.push_str(&format!(
                "     Bit Balance:    {:.2}% ones set\n",
                c.bit_balance_percent
            ));
            out.push_str(&format!("     Likely Type:    {}\n", c.likely_algorithm));
            out.push_str(&format!("     Hex Payload:    {}\n", c.hex_payload));
        }
    }

    out
}

pub fn format_unit_probe_json(report: &UnitProbeReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_bits() {
        assert_eq!(parse_key_bits("").unwrap(), 256);
        assert_eq!(parse_key_bits("128").unwrap(), 128);
        assert_eq!(parse_key_bits("256").unwrap(), 256);
        assert_eq!(parse_key_bits("512").unwrap(), 512);
        assert_eq!(parse_key_bits("16B").unwrap(), 128);
        assert_eq!(parse_key_bits("32B").unwrap(), 256);
        assert_eq!(parse_key_bits("64B").unwrap(), 512);
        assert_eq!(parse_key_bits("32 bytes").unwrap(), 256);
        assert_eq!(parse_key_bits("128-bit").unwrap(), 128);
        assert!(parse_key_bits("0").is_err());
        assert!(parse_key_bits("abc").is_err());
    }

    #[test]
    fn test_units_to_bytes() {
        let u8_units = vec![BigUint::from(0x12u32), BigUint::from(0x34u32)];
        assert_eq!(units_to_bytes(&u8_units, 8), vec![0x12, 0x34]);

        let u16_units = vec![BigUint::from(0x1234u32), BigUint::from(0x5678u32)];
        assert_eq!(units_to_bytes(&u16_units, 16), vec![0x12, 0x34, 0x56, 0x78]);

        let u4_units = vec![BigUint::from(0xAu32), BigUint::from(0xBu32)];
        assert_eq!(units_to_bytes(&u4_units, 4), vec![0xAB]);
    }

    #[test]
    fn test_find_crypto_keys_embedded() {
        // Stream: 100 null bytes + 32 distinct bytes (near max entropy) + 100 null bytes
        let mut data = vec![0u8; 100];
        let mut key = Vec::new();
        for i in 0..32u8 {
            key.push(i ^ 0xA5);
        }
        data.extend_from_slice(&key);
        data.extend_from_slice(&[0u8; 100]);

        let candidates = find_crypto_keys(&data, 8, 256);
        assert!(!candidates.is_empty());
        let top = &candidates[0];
        assert_eq!(top.bit_offset, 100 * 8);
        assert_eq!(top.start_unit, 100);
        assert_eq!(top.bit_length, 256);
        assert_eq!(top.byte_length, 32);
        assert!(top.normalized_entropy > 0.95);
        let expected_hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
        assert_eq!(top.hex_payload, expected_hex);
    }

    #[test]
    fn test_probe_unit_stream_zeroes() {
        let units = vec![BigUint::zero(); 100];
        let rep = probe_unit_stream(&units, 8, Some(256), "test_zeros");
        assert_eq!(rep.total_units, 100);
        assert_eq!(rep.distinct_units, 1);
        assert_eq!(rep.null_units, 100);
        assert_eq!(rep.unit_entropy, 0.0);
    }

    #[test]
    fn test_probe_unit_stream_stride() {
        // Periodic RGB stride: 1, 2, 3, 1, 2, 3...
        let mut units = Vec::new();
        for _ in 0..100 {
            units.push(BigUint::from(1u32));
            units.push(BigUint::from(2u32));
            units.push(BigUint::from(3u32));
        }
        let rep = probe_unit_stream(&units, 8, Some(256), "test_stride");
        assert!(rep.detected_strides.iter().any(|s| s.stride_units == 3));
    }
}
