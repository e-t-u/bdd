//! Multi-metric field collectors and stream profiling summaries.
//!
//! Evaluates statistical and information-theoretic metrics across tuple columns
//! and renders tabular or JSON profiling summaries.

use crate::analysis::accumulator::{create_accumulator, Accumulator};
use crate::error::BddError;
use crate::field::Field;
use serde::Serialize;

/// Summary profile metrics for a single field column.
#[derive(Debug, Clone, Serialize)]
pub struct FieldProfile {
    pub name: String,
    pub count: usize,
    pub min: String,
    pub max: String,
    pub avg: Option<f64>,
    pub shannon_entropy: f64,
    pub bit_balance_percent: f64,
    pub distinct_count: usize,
}

/// Collects multiple metrics for a single field column.
#[derive(Debug)]
pub struct FieldCollector {
    pub name: String,
    pub accumulators: Vec<Box<dyn Accumulator>>,
}

impl FieldCollector {
    /// Creates a collector equipped with the standard comprehensive set of metrics.
    pub fn new(name: &str) -> Self {
        let metrics = [
            "count", "min", "max", "avg", "entropy", "balance", "distinct",
        ];
        Self::with_metrics(name, &metrics).expect("standard metrics always valid")
    }

    /// Creates a collector equipped with custom specified metric names.
    pub fn with_metrics(name: &str, metric_names: &[&str]) -> Result<Self, BddError> {
        let mut accumulators = Vec::with_capacity(metric_names.len());
        for &m in metric_names {
            accumulators.push(create_accumulator(m)?);
        }
        Ok(Self {
            name: name.to_string(),
            accumulators,
        })
    }

    /// Ingest a field value into all attached accumulators.
    pub fn update(&mut self, field: &Field) {
        for acc in &mut self.accumulators {
            acc.update(field);
        }
    }

    /// Reset all attached accumulators.
    pub fn reset(&mut self) {
        for acc in &mut self.accumulators {
            acc.reset();
        }
    }

    /// Extract a structured `FieldProfile` from the current accumulator states.
    pub fn profile(&self) -> FieldProfile {
        let mut count = 0;
        let mut min_str = "-".to_string();
        let mut max_str = "-".to_string();
        let mut avg = None;
        let mut entropy = 0.0;
        let mut balance = 0.0;
        let mut distinct = 0;

        for acc in &self.accumulators {
            let val = acc.value();
            match acc.name() {
                "count" => count = val.as_u64() as usize,
                "min" => min_str = format!("{}", val),
                "max" => max_str = format!("{}", val),
                "avg" => avg = Some(val.as_f64()),
                "entropy" => entropy = val.as_f64(),
                "bit_balance" => balance = val.as_f64(),
                "distinct" => distinct = val.as_u64() as usize,
                _ => {}
            }
        }

        FieldProfile {
            name: self.name.clone(),
            count,
            min: min_str,
            max: max_str,
            avg,
            shannon_entropy: entropy,
            bit_balance_percent: balance,
            distinct_count: distinct,
        }
    }
}

/// Profiles a stream of tuples across multiple named or indexed columns.
#[derive(Debug)]
pub struct TupleCollector {
    columns: Vec<FieldCollector>,
}

impl TupleCollector {
    /// Creates a new tuple collector for a given schema of field names.
    pub fn new(field_names: &[String]) -> Self {
        let columns = field_names
            .iter()
            .map(|name| FieldCollector::new(name))
            .collect();
        Self { columns }
    }

    /// Creates an empty tuple collector with dynamic schema discovery on first tuple.
    pub fn empty() -> Self {
        Self {
            columns: Vec::new(),
        }
    }

    /// Creates a tuple collector from an initial tuple when explicit schema names are omitted.
    pub fn from_tuple(tuple: &[Field]) -> Self {
        let columns = (0..tuple.len())
            .map(|i| FieldCollector::new(&format!("field_{}", i)))
            .collect();
        let mut collector = Self { columns };
        collector.update(tuple);
        collector
    }

    /// Ingest an incoming tuple into the respective column collectors.
    pub fn update(&mut self, tuple: &[Field]) {
        // Expand columns if dynamically discovered tuples have more fields
        if self.columns.is_empty() && tuple.len() == 1 {
            self.columns.push(FieldCollector::new("unit"));
        } else {
            while self.columns.len() < tuple.len() {
                let idx = self.columns.len();
                self.columns
                    .push(FieldCollector::new(&format!("field_{}", idx)));
            }
        }

        for (i, field) in tuple.iter().enumerate() {
            self.columns[i].update(field);
        }
    }

    /// Extract field profiles for all columns.
    pub fn report(&self) -> Vec<FieldProfile> {
        self.columns.iter().map(|col| col.profile()).collect()
    }

    /// Renders an aligned ASCII summary table of field statistics.
    pub fn format_table(&self) -> String {
        let profiles = self.report();
        if profiles.is_empty() {
            return "No fields collected.\n".to_string();
        }

        let mut out = String::new();
        out.push_str(
            "Field         Count   Entropy (bits/B)   Bit 1s%   Distinct   Min           Max           Avg\n",
        );
        out.push_str(
            "-------------------------------------------------------------------------------------------------\n",
        );

        for p in &profiles {
            let avg_str = p
                .avg
                .map(|a| format!("{:.4}", a))
                .unwrap_or_else(|| "-".to_string());
            out.push_str(&format!(
                "{:<12} {:>6}   {:>16.4}   {:>6.2}%   {:>8}   {:<13} {:<13} {:<10}\n",
                p.name,
                p.count,
                p.shannon_entropy,
                p.bit_balance_percent,
                p.distinct_count,
                p.min,
                p.max,
                avg_str
            ));
        }

        out
    }

    /// Renders the profiles as formatted JSON.
    pub fn format_json(&self) -> String {
        let profiles = self.report();
        serde_json::to_string_pretty(&profiles).unwrap_or_else(|_| "[]".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigUint;

    #[test]
    fn test_tuple_collector_profiling() {
        let names = vec!["sync".to_string(), "pid".to_string()];
        let mut collector = TupleCollector::new(&names);

        let t1 = vec![
            Field::UInt(BigUint::from(0x47u32)),
            Field::UInt(BigUint::from(100u32)),
        ];
        let t2 = vec![
            Field::UInt(BigUint::from(0x47u32)),
            Field::UInt(BigUint::from(200u32)),
        ];

        collector.update(&t1);
        collector.update(&t2);

        let report = collector.report();
        assert_eq!(report.len(), 2);

        // sync column
        assert_eq!(report[0].name, "sync");
        assert_eq!(report[0].count, 2);
        assert_eq!(report[0].distinct_count, 1);
        assert_eq!(report[0].min, "71"); // 0x47 in decimal

        // pid column
        assert_eq!(report[1].name, "pid");
        assert_eq!(report[1].count, 2);
        assert_eq!(report[1].distinct_count, 2);
        assert_eq!(report[1].avg, Some(150.0));

        let table = collector.format_table();
        assert!(table.contains("sync"));
        assert!(table.contains("pid"));
    }
}
