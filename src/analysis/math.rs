//! Pure mathematical information-theoretic functions for binary analysis.
//!
//! Provides zero-dependency calculation of Shannon entropy, normalized entropy,
//! bit balance, Hamming weight, and byte distribution statistics.

/// Computes the Shannon entropy of a byte slice in bits per byte (0.0 to 8.0).
///
/// Shannon entropy is defined as:
/// \[
/// H(X) = -\sum_{i=0}^{255} p_i \log_2(p_i)
/// \]
/// where \( p_i \) is the relative frequency of byte \( i \).
///
/// An empty slice returns `0.0`.
pub fn shannon_entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut freq = [0usize; 256];
    for &b in bytes {
        freq[b as usize] += 1;
    }
    shannon_entropy_from_freq(&freq, bytes.len())
}

/// Computes Shannon entropy given pre-calculated symbol frequency counts and total count.
pub fn shannon_entropy_from_freq(freq: &[usize], total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }
    let total_f = total as f64;
    let mut h = 0.0f64;
    for &count in freq {
        if count > 0 {
            let p = count as f64 / total_f;
            h -= p * p.log2();
        }
    }
    h
}

/// Computes normalized Shannon entropy in the range [0.0, 1.0].
///
/// Normalizes entropy against the theoretical maximum for the given alphabet size:
/// \(\log_2(\min(N, 256))\).
pub fn normalized_entropy(bytes: &[u8]) -> f64 {
    let n = bytes.len();
    if n <= 1 {
        return 0.0;
    }
    let h = shannon_entropy(bytes);
    let max_h = (n as f64).min(256.0).log2();
    if max_h > 0.0 {
        (h / max_h).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// Computes the bit-level entropy of a sequence of bits in range [0.0, 1.0].
///
/// Based on binary entropy function \( H_b(p) = -p \log_2(p) - (1-p) \log_2(1-p) \).
pub fn bit_entropy(ones: usize, total_bits: usize) -> f64 {
    if total_bits == 0 || ones == 0 || ones >= total_bits {
        return 0.0;
    }
    let p = ones as f64 / total_bits as f64;
    let q = 1.0 - p;
    (-p * p.log2() - q * q.log2()).clamp(0.0, 1.0)
}

/// Computes the total Hamming weight (number of set 1-bits) in a byte slice.
pub fn hamming_weight(bytes: &[u8]) -> usize {
    bytes.iter().map(|b| b.count_ones() as usize).sum()
}

/// Computes the bit balance (percentage of set 1-bits, 0.0% to 100.0%).
///
/// A purely random stream will exhibit close to 50.0% bit balance.
pub fn bit_balance(bytes: &[u8], total_bits: usize) -> f64 {
    if total_bits == 0 {
        return 0.0;
    }
    let ones = hamming_weight(bytes);
    (ones as f64 / total_bits as f64) * 100.0
}

/// Computes the percentage of null (0x00) bytes in a byte slice (0.0% to 100.0%).
pub fn null_ratio(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let nulls = bytes.iter().filter(|&&b| b == 0).count();
    (nulls as f64 / bytes.len() as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_empty_and_uniform() {
        assert_eq!(shannon_entropy(&[]), 0.0);
        assert_eq!(normalized_entropy(&[]), 0.0);

        // Constant byte -> 0 entropy
        let zeroes = vec![0u8; 100];
        assert_eq!(shannon_entropy(&zeroes), 0.0);
        assert_eq!(normalized_entropy(&zeroes), 0.0);

        // Perfect 256-symbol uniform distribution -> exactly 8.0 bits
        let all_256: Vec<u8> = (0..=255).collect();
        let h = shannon_entropy(&all_256);
        assert!((h - 8.0).abs() < 1e-9, "Entropy was {}", h);
        assert!((normalized_entropy(&all_256) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_hamming_weight_and_bit_balance() {
        let bytes = [0xFF, 0x00, 0xAA, 0x55];
        // 0xFF = 8 ones, 0x00 = 0, 0xAA = 4, 0x55 = 4 -> total 16 ones in 32 bits
        assert_eq!(hamming_weight(&bytes), 16);
        assert_eq!(bit_balance(&bytes, 32), 50.0);
    }

    #[test]
    fn test_bit_entropy() {
        assert_eq!(bit_entropy(0, 100), 0.0);
        assert_eq!(bit_entropy(100, 100), 0.0);
        // 50% ones -> max bit entropy 1.0
        assert!((bit_entropy(50, 100) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_null_ratio() {
        let data = [0, 1, 0, 2];
        assert_eq!(null_ratio(&data), 50.0);
    }
}
