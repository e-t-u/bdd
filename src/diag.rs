//! Diagnostics and warning deduplication for high-throughput streams.
//!
//! Prevents `stderr` flooding on high-throughput or multi-gigabyte streams
//! by emitting diagnostic warnings on their first occurrence, suppressing repeated
//! notices during streaming, and summarizing deduplicated counts at EOF.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static QUIET: AtomicBool = AtomicBool::new(false);
static WARNING_COUNTS: Mutex<Option<HashMap<String, u64>>> = Mutex::new(None);

/// Set global quiet mode. When true, all diagnostic warnings are suppressed.
pub fn set_quiet(quiet: bool) {
    QUIET.store(quiet, Ordering::Relaxed);
}

/// Returns true if quiet mode is active.
pub fn is_quiet() -> bool {
    QUIET.load(Ordering::Relaxed)
}

/// Resets warning deduplication state.
pub fn reset() {
    if let Ok(mut guard) = WARNING_COUNTS.lock() {
        *guard = Some(HashMap::new());
    }
}

/// Emit a diagnostic warning with deduplication.
///
/// 1. If `--quiet` / `-q` is active, the warning is suppressed.
/// 2. If it is the first time this warning message is observed, it is printed immediately to `stderr`.
/// 3. If it has been emitted before, printing is suppressed and the occurrence counter is incremented.
pub fn warn<S: AsRef<str>>(msg: S) {
    if is_quiet() {
        return;
    }
    let msg_ref = msg.as_ref();
    let mut guard = match WARNING_COUNTS.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(msg_ref.to_string()).or_insert(0);
    *entry += 1;
    if *entry == 1 {
        eprintln!("{}", msg_ref);
    }
}

/// Returns the current count of observed occurrences for a warning message.
pub fn warning_count(msg: &str) -> u64 {
    let mut guard = match WARNING_COUNTS.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard
        .as_mut()
        .and_then(|map| map.get(msg).copied())
        .unwrap_or(0)
}

/// Summarize all deduplicated warnings at EOF / stream completion.
///
/// For any warning that occurred more than once, prints:
/// `[bdd] Warning: '<msg>' repeated <N> times`
///
/// Then clears the tracked warnings map.
pub fn flush_summary() {
    if is_quiet() {
        reset();
        return;
    }
    let mut guard = match WARNING_COUNTS.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(map) = guard.take() {
        let mut entries: Vec<(String, u64)> = map.into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (msg, count) in entries {
            if count > 1 {
                eprintln!("[bdd] Warning: '{}' repeated {} times", msg, count);
            }
        }
    }
}

/// Returns formatted summary strings for deduplicated warnings without clearing or printing them.
pub fn format_summaries() -> Vec<String> {
    if is_quiet() {
        return Vec::new();
    }
    let guard = match WARNING_COUNTS.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut out = Vec::new();
    if let Some(ref map) = *guard {
        let mut entries: Vec<(&String, &u64)> = map.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for (msg, &count) in entries {
            if count > 1 {
                out.push(format!("[bdd] Warning: '{}' repeated {} times", msg, count));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warning_deduplication() {
        set_quiet(false);
        reset();

        warn("Test warning 1");
        assert_eq!(warning_count("Test warning 1"), 1);

        warn("Test warning 1");
        assert_eq!(warning_count("Test warning 1"), 2);

        warn("Test warning 2");
        assert_eq!(warning_count("Test warning 2"), 1);

        let summaries = format_summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(
            summaries[0],
            "[bdd] Warning: 'Test warning 1' repeated 2 times"
        );

        flush_summary();
        assert_eq!(warning_count("Test warning 1"), 0);
    }

    #[test]
    fn test_quiet_mode_suppression() {
        reset();
        set_quiet(true);

        warn("Silenced warning");
        assert_eq!(warning_count("Silenced warning"), 0);
        assert!(format_summaries().is_empty());

        set_quiet(false);
    }
}
