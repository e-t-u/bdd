//! Foundational metric accumulators and collectors for streaming fields.
//!
//! Provides the core `Accumulator` trait and concrete implementations for
//! statistical reductions (count, sum, min, max, avg, variance) and
//! information-theoretic metrics (Shannon entropy, bit balance, distinct cardinality).

use crate::analysis::math::shannon_entropy_from_freq;
use crate::error::BddError;
use crate::field::{Field, FieldKey};
use num_bigint::{BigInt, BigUint};
use num_traits::Zero;
use std::collections::HashSet;

/// Interface for stateful metric accumulators processing sequences of fields.
pub trait Accumulator: std::fmt::Debug + Send + Sync {
    /// Ingest a single field into the running accumulator state.
    fn update(&mut self, field: &Field);

    /// Compute the current aggregated value as a `Field`.
    fn value(&self) -> Field;

    /// Reset the accumulator state to initial values.
    fn reset(&mut self);

    /// Canonical identifier for this metric (e.g. `"count"`, `"sum"`, `"avg"`, `"entropy"`).
    fn name(&self) -> &'static str;
}

/// Factory to instantiate an `Accumulator` by metric name.
///
/// Supported names:
/// - `"count"`: Count of elements
/// - `"sum"`: Running arithmetic sum
/// - `"min"`: Minimum element
/// - `"max"`: Maximum element
/// - `"avg"` | `"mean"`: Running arithmetic mean
/// - `"entropy"` | `"shannon_entropy"`: Shannon entropy of ingested bytes (0.0 to 8.0 bits/byte)
/// - `"balance"` | `"bit_balance"` | `"popcount"`: Percentage of set 1-bits (0.0% to 100.0%)
/// - `"variance"`: Online sample variance (Welford's algorithm)
/// - `"stddev"`: Sample standard deviation
/// - `"distinct"` | `"unique"`: Count of distinct values seen
pub fn create_accumulator(name: &str) -> Result<Box<dyn Accumulator>, BddError> {
    let normalized = name.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "count" => Ok(Box::new(CountAccumulator::new())),
        "sum" => Ok(Box::new(SumAccumulator::new())),
        "min" => Ok(Box::new(MinAccumulator::new())),
        "max" => Ok(Box::new(MaxAccumulator::new())),
        "avg" | "mean" => Ok(Box::new(MeanAccumulator::new())),
        "entropy" | "shannon_entropy" => Ok(Box::new(EntropyAccumulator::new())),
        "balance" | "bit_balance" | "popcount" => Ok(Box::new(BitBalanceAccumulator::new())),
        "variance" => Ok(Box::new(VarianceAccumulator::new(false))),
        "stddev" => Ok(Box::new(VarianceAccumulator::new(true))),
        "distinct" | "unique" => Ok(Box::new(DistinctAccumulator::new())),
        other => Err(BddError::CliError(format!(
            "Unknown accumulator metric '{}' (expected count, sum, min, max, avg, entropy, balance, variance, stddev, distinct)",
            other
        ))),
    }
}

/// Accumulator counting total elements ingested.
#[derive(Debug, Clone, Default)]
pub struct CountAccumulator {
    count: usize,
}

impl CountAccumulator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Accumulator for CountAccumulator {
    fn update(&mut self, _field: &Field) {
        self.count += 1;
    }

    fn value(&self) -> Field {
        Field::UInt(BigUint::from(self.count))
    }

    fn reset(&mut self) {
        self.count = 0;
    }

    fn name(&self) -> &'static str {
        "count"
    }
}

/// Accumulator computing the running arithmetic sum of numbers.
#[derive(Debug, Clone, Default)]
pub struct SumAccumulator {
    int_sum: BigInt,
    float_sum: f64,
    has_floats: bool,
}

impl SumAccumulator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Accumulator for SumAccumulator {
    fn update(&mut self, field: &Field) {
        match field {
            Field::Float(f) => {
                self.has_floats = true;
                self.float_sum += f;
                self.int_sum += BigInt::from(*f as i64);
            }
            _ => {
                self.int_sum += field.as_bigint();
                self.float_sum += field.as_f64();
            }
        }
    }

    fn value(&self) -> Field {
        if self.has_floats {
            Field::Float(self.float_sum)
        } else {
            Field::Int(self.int_sum.clone())
        }
    }

    fn reset(&mut self) {
        self.int_sum = BigInt::zero();
        self.float_sum = 0.0;
        self.has_floats = false;
    }

    fn name(&self) -> &'static str {
        "sum"
    }
}

/// Accumulator tracking the minimum value seen.
#[derive(Debug, Clone, Default)]
pub struct MinAccumulator {
    min: Option<Field>,
}

impl MinAccumulator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Accumulator for MinAccumulator {
    fn update(&mut self, field: &Field) {
        match &self.min {
            None => self.min = Some(field.clone()),
            Some(curr) => {
                if field < curr {
                    self.min = Some(field.clone());
                }
            }
        }
    }

    fn value(&self) -> Field {
        self.min.clone().unwrap_or(Field::UInt(BigUint::zero()))
    }

    fn reset(&mut self) {
        self.min = None;
    }

    fn name(&self) -> &'static str {
        "min"
    }
}

/// Accumulator tracking the maximum value seen.
#[derive(Debug, Clone, Default)]
pub struct MaxAccumulator {
    max: Option<Field>,
}

impl MaxAccumulator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Accumulator for MaxAccumulator {
    fn update(&mut self, field: &Field) {
        match &self.max {
            None => self.max = Some(field.clone()),
            Some(curr) => {
                if field > curr {
                    self.max = Some(field.clone());
                }
            }
        }
    }

    fn value(&self) -> Field {
        self.max.clone().unwrap_or(Field::UInt(BigUint::zero()))
    }

    fn reset(&mut self) {
        self.max = None;
    }

    fn name(&self) -> &'static str {
        "max"
    }
}

/// Accumulator computing the running arithmetic mean.
#[derive(Debug, Clone, Default)]
pub struct MeanAccumulator {
    count: usize,
    sum: f64,
}

impl MeanAccumulator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Accumulator for MeanAccumulator {
    fn update(&mut self, field: &Field) {
        self.count += 1;
        self.sum += field.as_f64();
    }

    fn value(&self) -> Field {
        if self.count == 0 {
            Field::Float(0.0)
        } else {
            Field::Float(self.sum / self.count as f64)
        }
    }

    fn reset(&mut self) {
        self.count = 0;
        self.sum = 0.0;
    }

    fn name(&self) -> &'static str {
        "avg"
    }
}

/// Accumulator tracking byte-level Shannon entropy (0.0 to 8.0 bits/byte) across ingested fields.
#[derive(Debug, Clone)]
pub struct EntropyAccumulator {
    freq: [usize; 256],
    total_bytes: usize,
}

impl Default for EntropyAccumulator {
    fn default() -> Self {
        Self {
            freq: [0usize; 256],
            total_bytes: 0,
        }
    }
}

impl EntropyAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the raw byte frequency distribution.
    pub fn frequencies(&self) -> &[usize; 256] {
        &self.freq
    }

    /// Return the total number of bytes ingested.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }
}

impl Accumulator for EntropyAccumulator {
    fn update(&mut self, field: &Field) {
        let bytes = field.to_bytes();
        for &b in &bytes {
            self.freq[b as usize] += 1;
        }
        self.total_bytes += bytes.len();
    }

    fn value(&self) -> Field {
        let h = shannon_entropy_from_freq(&self.freq, self.total_bytes);
        Field::Float(h)
    }

    fn reset(&mut self) {
        self.freq = [0usize; 256];
        self.total_bytes = 0;
    }

    fn name(&self) -> &'static str {
        "entropy"
    }
}

/// Accumulator tracking bit balance (% of 1-bits) across ingested fields.
#[derive(Debug, Clone, Default)]
pub struct BitBalanceAccumulator {
    total_bits: usize,
    ones_count: usize,
}

impl BitBalanceAccumulator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Accumulator for BitBalanceAccumulator {
    fn update(&mut self, field: &Field) {
        let bits = field.bit_length();
        let ones = field.hamming_weight();
        self.total_bits += bits;
        self.ones_count += ones;
    }

    fn value(&self) -> Field {
        if self.total_bits == 0 {
            Field::Float(0.0)
        } else {
            let pct = (self.ones_count as f64 / self.total_bits as f64) * 100.0;
            Field::Float(pct)
        }
    }

    fn reset(&mut self) {
        self.total_bits = 0;
        self.ones_count = 0;
    }

    fn name(&self) -> &'static str {
        "bit_balance"
    }
}

/// Accumulator computing running sample variance and standard deviation using Welford's algorithm.
#[derive(Debug, Clone)]
pub struct VarianceAccumulator {
    count: usize,
    mean: f64,
    m2: f64,
    emit_stddev: bool,
}

impl VarianceAccumulator {
    pub fn new(emit_stddev: bool) -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
            emit_stddev,
        }
    }

    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    pub fn stddev(&self) -> f64 {
        self.variance().sqrt()
    }
}

impl Accumulator for VarianceAccumulator {
    fn update(&mut self, field: &Field) {
        let x = field.as_f64();
        self.count += 1;
        let delta = x - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
    }

    fn value(&self) -> Field {
        if self.emit_stddev {
            Field::Float(self.stddev())
        } else {
            Field::Float(self.variance())
        }
    }

    fn reset(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
    }

    fn name(&self) -> &'static str {
        if self.emit_stddev {
            "stddev"
        } else {
            "variance"
        }
    }
}

/// Accumulator counting distinct field values seen.
#[derive(Debug, Clone, Default)]
pub struct DistinctAccumulator {
    seen: HashSet<FieldKey>,
}

impl DistinctAccumulator {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Accumulator for DistinctAccumulator {
    fn update(&mut self, field: &Field) {
        self.seen.insert(field.to_key());
    }

    fn value(&self) -> Field {
        Field::UInt(BigUint::from(self.seen.len()))
    }

    fn reset(&mut self) {
        self.seen.clear();
    }

    fn name(&self) -> &'static str {
        "distinct"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_and_sum_accumulators() {
        let mut count = CountAccumulator::new();
        let mut sum = SumAccumulator::new();

        for i in 1..=5 {
            let f = Field::UInt(BigUint::from(i as u32));
            count.update(&f);
            sum.update(&f);
        }

        assert_eq!(count.value(), Field::UInt(BigUint::from(5u32)));
        assert_eq!(sum.value(), Field::Int(BigInt::from(15)));

        count.reset();
        assert_eq!(count.value(), Field::UInt(BigUint::zero()));
    }

    #[test]
    fn test_min_max_mean_accumulators() {
        let mut min = MinAccumulator::new();
        let mut max = MaxAccumulator::new();
        let mut avg = MeanAccumulator::new();

        let values = [10, 5, 20, 15];
        for &v in &values {
            let f = Field::UInt(BigUint::from(v as u32));
            min.update(&f);
            max.update(&f);
            avg.update(&f);
        }

        assert_eq!(min.value(), Field::UInt(BigUint::from(5u32)));
        assert_eq!(max.value(), Field::UInt(BigUint::from(20u32)));
        assert_eq!(avg.value(), Field::Float(12.5));
    }

    #[test]
    fn test_entropy_and_bit_balance_accumulators() {
        let mut entropy = EntropyAccumulator::new();
        let mut balance = BitBalanceAccumulator::new();

        // 256 distinct bytes -> 8.0 bits entropy
        let all_bytes: Vec<u8> = (0..=255).collect();
        let f = Field::Bytes(all_bytes);
        entropy.update(&f);
        balance.update(&f);

        if let Field::Float(h) = entropy.value() {
            assert!((h - 8.0).abs() < 1e-9, "Expected 8.0 entropy, got {}", h);
        } else {
            panic!("Expected float field");
        }

        if let Field::Float(pct) = balance.value() {
            assert!(
                (pct - 50.0).abs() < 1e-9,
                "Expected 50% balance, got {}",
                pct
            );
        } else {
            panic!("Expected float field");
        }
    }

    #[test]
    fn test_variance_and_distinct_accumulators() {
        let mut var = VarianceAccumulator::new(false);
        let mut distinct = DistinctAccumulator::new();

        let values = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        for &v in &values {
            let f = Field::Float(v);
            var.update(&f);
            distinct.update(&f);
        }

        // Variance of sample [2, 4, 4, 4, 5, 5, 7, 9] is 4.5714...
        if let Field::Float(v) = var.value() {
            assert!((v - 4.57142857).abs() < 1e-4, "Got variance {}", v);
        }
        assert_eq!(distinct.value(), Field::UInt(BigUint::from(5u32)));
    }

    #[test]
    fn test_create_accumulator_factory() {
        let acc = create_accumulator("entropy").unwrap();
        assert_eq!(acc.name(), "entropy");

        let mut count = create_accumulator("count").unwrap();
        count.update(&Field::UInt(BigUint::from(42u32)));
        assert_eq!(count.value(), Field::UInt(BigUint::from(1u32)));

        assert!(create_accumulator("unknown_metric").is_err());
    }
}
