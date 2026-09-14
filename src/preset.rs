//! Binary format presets and schemas for bdd.
//!
//! Presets provide immediate unaligned bit patterns, unit sizing, endianness, and field names
//! for real-world multimedia containers, AI weight quantization formats, and network headers.
//!
//! Presets are stored in an external `presets.json` file. If missing, the default presets
//! are seeded from embedded defaults. A custom presets file can be specified via
//! `--presets-file <PATH>` or `BDD_PRESETS_PATH`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;

/// Embedded fallback presets JSON in case of offline first-run.
pub const DEFAULT_PRESETS_JSON: &str = include_str!("../presets.json");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub description: String,
    pub pattern: String,
    pub unit_bits: usize,
    pub little_endian: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_count: Option<u64>,
}

static CUSTOM_PRESETS_PATH: RwLock<Option<PathBuf>> = RwLock::new(None);
static GLOBAL_PRESETS: RwLock<Option<&'static [Preset]>> = RwLock::new(None);

/// Set an explicit custom path to the presets file and reload cache.
pub fn set_custom_presets_path(path: PathBuf) {
    let mut p = CUSTOM_PRESETS_PATH.write().unwrap();
    *p = Some(path);
    drop(p);
    reload_presets();
}

/// Reset custom presets path override.
pub fn reset_custom_presets_path() {
    let mut p = CUSTOM_PRESETS_PATH.write().unwrap();
    *p = None;
    drop(p);
    reload_presets();
}

/// Returns the user-writable presets file path for downloads and overrides:
/// 1. Explicit override (`set_custom_presets_path` / `--presets-file`)
/// 2. `BDD_PRESETS_FILE` environment variable
/// 3. User config: `$XDG_CONFIG_HOME/bdd/presets.json` or `~/.config/bdd/presets.json`
pub fn get_user_presets_file_path() -> PathBuf {
    if let Some(ref p) = *CUSTOM_PRESETS_PATH.read().unwrap() {
        return p.clone();
    }
    if let Ok(env_path) = std::env::var("BDD_PRESETS_FILE") {
        if !env_path.trim().is_empty() {
            return PathBuf::from(env_path);
        }
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.trim().is_empty() {
            return PathBuf::from(xdg).join("bdd").join("presets.json");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home)
                .join(".config")
                .join("bdd")
                .join("presets.json");
        }
    }
    PathBuf::from("presets.json")
}

/// Find an existing presets file path according to standard FHS and XDG discovery order:
/// 1. Explicit override (`set_custom_presets_path` / `--presets-file`)
/// 2. `BDD_PRESETS_FILE` environment variable
/// 3. Local `./presets.json` in current working directory
/// 4. User config: `$XDG_CONFIG_HOME/bdd/presets.json` or `~/.config/bdd/presets.json`
/// 5. System package shared data: `/usr/share/bdd/presets.json` (Debian & RPM packages)
/// 6. System local shared data: `/usr/local/share/bdd/presets.json`
/// 7. System configuration: `/etc/bdd/presets.json`
pub fn find_existing_presets_file() -> Option<PathBuf> {
    if let Some(ref p) = *CUSTOM_PRESETS_PATH.read().unwrap() {
        if p.is_file() {
            return Some(p.clone());
        }
    }
    if let Ok(env_path) = std::env::var("BDD_PRESETS_FILE") {
        let p = PathBuf::from(env_path);
        if p.is_file() {
            return Some(p);
        }
    }
    let cwd_file = PathBuf::from("presets.json");
    if cwd_file.is_file() {
        return Some(cwd_file);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg).join("bdd").join("presets.json");
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home)
            .join(".config")
            .join("bdd")
            .join("presets.json");
        if p.is_file() {
            return Some(p);
        }
    }
    for sys_path in &[
        "/usr/share/bdd/presets.json",
        "/usr/local/share/bdd/presets.json",
        "/etc/bdd/presets.json",
    ] {
        let p = PathBuf::from(sys_path);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Determine active presets file path (either existing found file or default user path).
pub fn get_presets_file_path() -> PathBuf {
    find_existing_presets_file().unwrap_or_else(get_user_presets_file_path)
}

fn filter_feature_presets(presets: Vec<Preset>) -> Vec<Preset> {
    #[cfg(not(feature = "small-floats"))]
    {
        const SMALL_FLOAT_NAMES: &[&str] =
            &["nvfp4", "fp6-e3m2", "fp8-e4m3", "fp8-e5m2", "bf16", "fp16"];
        presets
            .into_iter()
            .filter(|p| !SMALL_FLOAT_NAMES.contains(&p.name.as_str()))
            .collect()
    }
    #[cfg(feature = "small-floats")]
    {
        presets
    }
}

/// Internal helper to load presets from file or automatically download/seed if missing.
fn load_presets() -> Vec<Preset> {
    // 1. If an existing presets file is found on filesystem, load and parse it
    if let Some(path) = find_existing_presets_file() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<Vec<Preset>>(&content) {
                return filter_feature_presets(parsed);
            } else {
                eprintln!(
                    "[bdd] Warning: Failed to parse presets file at '{}'. Falling back to defaults.",
                    path.display()
                );
            }
        }
    }

    let user_path = get_user_presets_file_path();

    // 2. File does not exist: seed user presets file from embedded defaults and return
    if let Some(parent) = user_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&user_path, DEFAULT_PRESETS_JSON);
    let parsed: Vec<Preset> = serde_json::from_str(DEFAULT_PRESETS_JSON).unwrap_or_default();
    filter_feature_presets(parsed)
}

/// Force reloading presets from disk into global static cache.
pub fn reload_presets() {
    let mut write_guard = GLOBAL_PRESETS.write().unwrap();
    let loaded = load_presets();
    let leaked: &'static [Preset] = Box::leak(loaded.into_boxed_slice());
    *write_guard = Some(leaked);
}

/// Returns all available presets.
pub fn all_presets() -> &'static [Preset] {
    let read_guard = GLOBAL_PRESETS.read().unwrap();
    if let Some(slice) = *read_guard {
        return slice;
    }
    drop(read_guard);

    let mut write_guard = GLOBAL_PRESETS.write().unwrap();
    if let Some(slice) = *write_guard {
        return slice;
    }
    let loaded = load_presets();
    let leaked: &'static [Preset] = Box::leak(loaded.into_boxed_slice());
    *write_guard = Some(leaked);
    leaked
}

/// Find a preset by name (case-insensitive, allows hyphens and underscores).
pub fn find_preset(name: &str) -> Option<&'static Preset> {
    let normalized = name.to_lowercase().replace('_', "-");
    all_presets()
        .iter()
        .find(|p| p.name.to_lowercase().replace('_', "-") == normalized)
}

/// Formats the list of available presets as an aligned human-readable table.
pub fn format_presets_table() -> String {
    let mut out = String::new();
    out.push_str("Available bdd Binary Format Presets:\n");
    out.push_str(&format!(
        "{:<19}  {:<8}  {:<50}  {}\n",
        "NAME", "UNIT", "DESCRIPTION", "PATTERN"
    ));
    out.push_str(&format!(
        "{:-<19}  {:-<8}  {:-<50}  {:-<30}\n",
        "", "", "", ""
    ));
    for p in all_presets() {
        let unit_str = format!("{}b ({}B)", p.unit_bits, p.unit_bits / 8);
        out.push_str(&format!(
            "{:<19}  {:<8}  {:<50}  {}\n",
            p.name, unit_str, p.description, p.pattern
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_embedded_default_presets_valid() {
        let parsed: Vec<Preset> = serde_json::from_str(DEFAULT_PRESETS_JSON).expect("valid JSON");
        assert!(parsed.len() >= 19);
        let names: Vec<_> = parsed.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"mp3-header"));
        assert!(names.contains(&"mpeg-ts"));
        assert!(names.contains(&"ipv4-header"));
    }

    #[test]
    fn test_find_preset_normalization() {
        let _guard = TEST_MUTEX.lock().unwrap();
        reset_custom_presets_path();
        let p1 = find_preset("mp3-header").expect("found mp3-header");
        let p2 = find_preset("MP3_HEADER").expect("found MP3_HEADER");
        assert_eq!(p1.name, p2.name);
        assert_eq!(p1.unit_bits, 32);
    }

    #[test]
    fn test_custom_presets_path() {
        let _guard = TEST_MUTEX.lock().unwrap();
        use tempfile::NamedTempFile;

        let sample_json = r#"[
            {
                "name": "custom-proto",
                "description": "Custom test protocol header",
                "pattern": "magic:16u,len:16u",
                "unit_bits": 32,
                "little_endian": false,
                "default_count": 1
            }
        ]"#;

        let src_file = NamedTempFile::new().unwrap();
        std::fs::write(src_file.path(), sample_json).unwrap();

        set_custom_presets_path(src_file.path().to_path_buf());

        let p = find_preset("custom-proto").expect("find custom-proto");
        assert_eq!(p.unit_bits, 32);
        assert_eq!(p.pattern, "magic:16u,len:16u");

        reset_custom_presets_path();
    }
}
