//! Foundational statistical, information-theoretic, and stream metric analysis.
//!
//! Exposes:
//! - Pure math: [`shannon_entropy`], [`normalized_entropy`], [`bit_balance`], [`hamming_weight`].
//! - Stateful metric collectors: [`Accumulator`], [`CountAccumulator`], [`SumAccumulator`],
//!   [`MinAccumulator`], [`MaxAccumulator`], [`MeanAccumulator`], [`EntropyAccumulator`],
//!   [`BitBalanceAccumulator`], [`VarianceAccumulator`], [`DistinctAccumulator`].
//! - Profiling summaries: [`FieldCollector`], [`TupleCollector`], [`FieldProfile`].

pub mod accumulator;
pub mod math;
pub mod profile;

pub use accumulator::{
    create_accumulator, Accumulator, BitBalanceAccumulator, CountAccumulator, DistinctAccumulator,
    EntropyAccumulator, MaxAccumulator, MeanAccumulator, MinAccumulator, SumAccumulator,
    VarianceAccumulator,
};
pub use math::{
    bit_balance, bit_entropy, hamming_weight, normalized_entropy, null_ratio, shannon_entropy,
    shannon_entropy_from_freq,
};
pub use profile::{FieldCollector, FieldProfile, TupleCollector};
