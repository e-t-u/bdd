use crate::error::BddError;
use clap::Parser;

/// Command line arguments for the bdd bitstream utility.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "bdd",
    version = env!("CARGO_PKG_VERSION"),
    about = "Unix command line program to handle bit streams. For AI model prompt rules run: bdd --llms",
    after_help = "AI & LLM Integration:\n  Run 'bdd --llms' (or 'bdd --ai-guide') to print the concise agent cheatsheet.\n  Run 'bdd --mcp' to launch the Model Context Protocol stdio server.\n  Documentation file: /usr/share/doc/bdd/llms.txt"
)]
pub struct Cli {
    // Positional pattern arguments
    /// Stream I/O pipeline or pattern (e.g. '8->3', '4U4U -> {0|1} -> hex', '188B[11:13] -> 13', '4U4U')
    #[arg(value_name = "STREAM_PATTERN")]
    pub stream_pattern: Option<String>,

    // File options
    #[arg(long, default_value = "-")]
    pub input_file: String,

    #[arg(long, default_value = "-")]
    pub output_file: String,

    // Input unit and container selection
    /// Size of active unit in bits (interesting bits inside container)
    #[arg(long)]
    pub input_unit: Option<String>,

    /// Size of repeating container in bits
    #[arg(long)]
    pub input_raw_unit: Option<String>,

    /// Initial bit offset (skip) before first container
    #[arg(long, allow_hyphen_values = true)]
    pub input_skip_bits: Option<String>,

    #[arg(long, allow_hyphen_values = true)]
    pub input_skip_units: Option<String>,

    /// Bit gap between repeating containers (or between units)
    #[arg(long, allow_hyphen_values = true)]
    pub input_gap: Option<String>,

    /// Bit offset (pregap) of unit inside container
    #[arg(long, allow_hyphen_values = true)]
    pub input_offset: Option<String>,

    #[arg(long, default_value_t = false)]
    pub input_assert_aligned: bool,

    /// Discard incomplete trailing bits at EOF instead of zero-padding
    #[arg(
        long,
        default_value_t = false,
        visible_aliases = ["drop-partial-eof", "drop-trailing-bits", "no-pad-eof"]
    )]
    pub input_drop_partial_eof: bool,

    /// Disable seeking on all inputs (force streaming sequential read)
    #[arg(
        long,
        default_value_t = false,
        visible_aliases = [
            "do-not-seek",
            "no-input-seek",
            "input-no-seek",
            "no-merge-seek",
            "merge-no-seek"
        ]
    )]
    pub no_seek: bool,

    /// Enable seeking on primary input (default: true if seekable)
    #[arg(
        long,
        default_value_t = false,
        visible_aliases = ["input-seek", "merge-seek", "merge-use-seek", "use-seek", "seek"]
    )]
    pub input_use_seek: bool,

    /// Force disable memory-mapped I/O on all input files (use standard buffered reads)
    #[arg(
        long,
        default_value_t = false,
        visible_aliases = [
            "no-mmap",
            "do-not-mmap",
            "no-input-mmap",
            "input-no-mmap",
            "no-merge-mmap",
            "merge-no-mmap"
        ]
    )]
    pub no_mmap: bool,

    /// Enable memory-mapped I/O on input files (default: true if regular file and supported)
    #[arg(
        long,
        default_value_t = false,
        visible_aliases = ["input-mmap", "merge-mmap", "input-use-mmap", "merge-use-mmap"]
    )]
    pub mmap: bool,

    #[arg(long, default_value_t = false, visible_aliases = ["little-endian"])]
    pub input_little_endian: bool,

    #[arg(long, default_value_t = false, visible_aliases = ["reverse-input-bytes", "reverse-bytes"])]
    pub input_reverse_bytes: bool,

    #[arg(long, default_value_t = false, visible_aliases = ["reverse-input-units", "reverse-input-unit", "reverse-unit", "reverse-units"])]
    pub input_reverse_unit: bool,

    // Special input bit streams
    /// Generate endless stream of zero bits
    #[arg(long, default_value_t = false)]
    pub input_zeros: bool,

    /// Generate endless stream of one bits
    #[arg(long, default_value_t = false)]
    pub input_ones: bool,

    /// Generate random bits from /dev/urandom
    #[arg(long, default_value_t = false)]
    pub input_random: bool,

    /// Generate sequential counter numbers (0, 1, 2...)
    #[arg(long, default_value_t = false)]
    pub input_counter: bool,

    /// Read text input as newline-separated unsigned integers
    #[arg(long, default_value_t = false)]
    pub input_integers: bool,

    // Tuples
    #[arg(short = 'p', long)]
    pub input_pattern: Option<String>,

    #[arg(long, default_value_t = false)]
    pub input_tuples: bool,

    #[arg(long, allow_hyphen_values = true)]
    pub skip: Option<String>,

    #[arg(long, allow_hyphen_values = true)]
    pub count: Option<String>,

    // Manipulate tuples
    /// Construct result tuple by listing input field indices in desired output order (e.g. "1,0,2", "-1,0", "0,0")
    #[arg(long, default_missing_value = "", num_args = 0..=1)]
    pub rearrange: Option<String>,

    /// Clamp or bound field F to a range (e.g. "0,255", "0,-100,100:saturate", "0,255:wrap")
    #[arg(long, allow_hyphen_values = true)]
    pub clamp: Option<String>,

    /// Round, bound, or clamp field F (modes: saturate, wrap, zero, drop, trunc, floor, ceil, round, round_ties_even)
    #[arg(long, visible_alias = "cut-maxint")]
    pub round: Option<String>,

    #[arg(long)]
    pub remove_right: Option<String>,

    #[arg(long)]
    pub shift_right: Option<String>,

    #[arg(long)]
    pub shift_left: Option<String>,

    #[arg(long)]
    pub xor: Option<String>,

    #[arg(long)]
    pub and: Option<String>,

    #[arg(long)]
    pub or: Option<String>,

    #[arg(long)]
    pub not: Option<String>,

    #[arg(long)]
    pub abs: Option<String>,

    #[arg(long)]
    pub sign: Option<String>,

    #[arg(long)]
    pub add: Option<String>,

    #[arg(long)]
    pub sub: Option<String>,

    #[arg(long)]
    pub mul: Option<String>,

    #[arg(long)]
    pub div: Option<String>,

    #[arg(long)]
    pub r#mod: Option<String>,

    #[arg(long)]
    pub filter: Option<String>,

    /// Set field value directly to a constant (syntax: [field,]value)
    #[arg(long)]
    pub set: Option<String>,

    // Pack tuples
    #[arg(short = 'P', long)]
    pub output_pattern: Option<String>,

    #[arg(long, visible_alias = "output-tuple", default_value_t = false)]
    pub output_tuples: bool,

    // Output unit and framing options
    #[arg(long)]
    pub output_unit: Option<String>,

    /// Size of repeating output container in bits
    #[arg(long)]
    pub output_raw_unit: Option<String>,

    /// Bit offset (pregap) of unit inside output container
    #[arg(long, allow_hyphen_values = true)]
    pub output_offset: Option<String>,

    /// Bit gap between output containers
    #[arg(long, allow_hyphen_values = true)]
    pub output_gap: Option<String>,

    /// Initial bit offset (skip/prefix) emitted on output before first container
    #[arg(long, allow_hyphen_values = true, visible_aliases = ["output-skip", "output-prefix"])]
    pub output_skip_bits: Option<String>,

    #[arg(long, default_value_t = false)]
    pub output_little_endian: bool,

    #[arg(long, default_value_t = false, visible_aliases = ["reverse-output-bytes"])]
    pub output_reverse_bytes: bool,

    #[arg(long, default_value_t = false, visible_aliases = ["reverse-output-units", "reverse-output-unit"])]
    pub output_reverse_unit: bool,

    // Special output formats
    #[arg(long, visible_alias = "output-integer", default_value_t = false)]
    pub output_integers: bool,

    #[arg(long, default_value_t = false)]
    pub output_hex: bool,

    #[arg(long, visible_alias = "output-bit", default_value_t = false)]
    pub output_bits: bool,

    /// Preserve container pre-offset and post-offset bits, skips, and gaps "as is" from original input instead of zeroing
    #[arg(long, default_value_t = false, visible_aliases = ["as-is", "as_is"])]
    pub overwrite: bool,

    #[arg(long, default_value_t = false)]
    pub output_json: bool,

    #[arg(long, default_value_t = false)]
    pub output_csv: bool,

    #[arg(long)]
    pub csv_header: Option<String>,

    #[arg(long, default_value_t = false)]
    pub output_visual: bool,

    /// Emit JSON tuples as key-value objects instead of arrays
    #[arg(long, default_value_t = false, visible_alias = "output-json-dict")]
    pub json_object: bool,

    /// Comma-separated field names for JSON object / CSV output
    #[arg(long, visible_alias = "json-keys")]
    pub json_fields: Option<String>,
    /// Use built-in format preset (e.g. 'mp3-header', 'mpeg-ts', 'nvfp4', 'wav-header')
    #[arg(long)]
    pub preset: Option<String>,

    /// List all built-in format presets and exit
    #[arg(long, default_value_t = false)]
    pub list_presets: bool,

    /// Path to presets JSON file (overrides ~/.config/bdd/presets.json or ./presets.json)
    #[arg(long, value_name = "PATH", visible_alias = "presets-path")]
    pub presets_file: Option<std::path::PathBuf>,

    /// Explain pattern layout, field bit ranges, and byte alignment
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    pub explain_pattern: Option<String>,

    /// Export C packed struct definition (DEPRECATED: use 'bdd --explain-pattern --output-json | python3 contrib/python/json_to_c.py')
    #[arg(long, default_value_t = false, visible_aliases = ["c-struct", "to-c"])]
    pub export_c: bool,

    /// Export Rust struct definition (DEPRECATED: use 'bdd --explain-pattern --output-json | python3 contrib/python/json_to_rust.py')
    #[arg(long, default_value_t = false, visible_aliases = ["rust-struct", "to-rust"])]
    pub export_rust: bool,

    /// Probe binary file characteristics, Shannon entropy, and repeating strides
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    pub probe: Option<String>,

    /// Probe unit stream characteristics and entropy AFTER input stream processing
    #[arg(long, default_value_t = false, visible_alias = "probe-stream")]
    pub probe_units: bool,

    /// Scan unit stream for potential maximum-entropy cryptographic keys (e.g. 128, 256, 512 bits)
    #[arg(long, num_args = 0..=1, default_missing_value = "256", visible_aliases = ["probe-crypto-keys", "probe-key"])]
    pub probe_keys: Option<String>,

    /// Specific tuple field index (0-based) to probe when using stream patterns
    #[arg(long)]
    pub probe_field: Option<usize>,

    /// Display visual entropy sparkline map during probing
    #[arg(long, default_value_t = false, visible_aliases = ["probe-visual", "probe-map"])]
    pub probe_visual: bool,

    /// Generate shell auto-completion script (bash, zsh, fish, elvish, powershell)
    #[arg(long, value_enum, value_name = "SHELL")]
    pub completions: Option<clap_complete::Shell>,

    /// Start Model Context Protocol (MCP) JSON-RPC 2.0 stdio server
    #[arg(long, default_value_t = false)]
    pub mcp: bool,

    /// Print concise AI model and agent guide (llms.txt) to stdout
    #[arg(long, default_value_t = false, visible_aliases = ["ai-guide", "ai"])]
    pub llms: bool,

    /// Suppress non-fatal diagnostic warnings and deduplication summaries
    #[arg(short = 'q', long = "quiet", default_value_t = false)]
    pub quiet: bool,

    // Demux and looping options
    #[arg(long)]
    pub demux: Vec<String>,

    #[arg(long)]
    pub demux_files: Option<String>,

    /// Repeat input stream N times (0 = infinite)
    #[arg(long, visible_alias = "repeat-input")]
    pub input_repeat: Option<String>,

    // Merge options
    /// Merge file(s) to interleave round-robin with primary stream (repeatable)
    #[arg(long, action = clap::ArgAction::Append)]
    pub merge_file: Vec<String>,

    /// Comma- or space-separated list of merge files to interleave round-robin
    #[arg(long, value_delimiter = ',')]
    pub merge_files: Vec<String>,

    #[arg(long)]
    pub merge_unit: Option<String>,

    #[arg(long, allow_hyphen_values = true)]
    pub merge_skip_bits: Option<String>,

    #[arg(long, allow_hyphen_values = true)]
    pub merge_skip_units: Option<String>,

    #[arg(long, allow_hyphen_values = true)]
    pub merge_copy_first: Option<String>,

    /// Size of repeating container in bits for merge stream
    #[arg(long)]
    pub merge_raw_unit: Option<String>,

    /// Bit gap between merge containers
    #[arg(long, allow_hyphen_values = true)]
    pub merge_gap: Option<String>,

    /// Bit offset (pregap) of unit inside merge container
    #[arg(long, allow_hyphen_values = true)]
    pub merge_offset: Option<String>,

    #[arg(long, default_value_t = false)]
    pub merge_assert_aligned: bool,

    /// Discard incomplete trailing bits at EOF in merge stream instead of zero-padding
    #[arg(
        long,
        default_value_t = false,
        visible_aliases = ["merge-drop-trailing-bits", "merge-no-pad-eof"]
    )]
    pub merge_drop_partial_eof: bool,

    #[arg(long, default_value_t = false)]
    pub merge_little_endian: bool,

    #[arg(long, default_value_t = false)]
    pub merge_reverse_bytes: bool,

    #[arg(long, default_value_t = false)]
    pub merge_reverse_unit: bool,
}

/// Fully validated execution plan derived from CLI arguments.
#[derive(Debug, Clone)]
pub struct ValidatedConfig {
    pub input_file: String,
    pub output_file: String,
    pub input_unit: Option<usize>,
    pub input_raw_unit: Option<u64>,
    pub input_offset: u64,
    pub input_skip_bits: u64,
    pub input_skip_units: u64,
    pub input_gap: u64,
    pub input_assert_aligned: bool,
    pub input_drop_partial_eof: bool,
    pub input_use_seek: bool,
    pub input_use_mmap: bool,
    pub input_reverse_bytes: bool,
    pub input_reverse_unit: bool,
    pub input_zeros: bool,
    pub input_ones: bool,
    pub input_random: bool,
    pub input_counter: bool,
    pub input_integers: bool,
    pub input_pattern: Option<String>,
    pub input_tuples: bool,
    pub skip: u64,
    pub count: Option<u64>,
    pub rearrange: Option<String>,
    pub clamp: Option<String>,
    pub round: Option<String>,
    pub cut_maxint: Option<String>,
    pub remove_right: Option<String>,
    pub shift_right: Option<String>,
    pub shift_left: Option<String>,
    pub xor: Option<String>,
    pub and: Option<String>,
    pub or: Option<String>,
    pub not: Option<String>,
    pub abs: Option<String>,
    pub sign: Option<String>,
    pub add: Option<String>,
    pub sub: Option<String>,
    pub mul: Option<String>,
    pub div: Option<String>,
    pub r#mod: Option<String>,
    pub filter: Option<String>,
    pub set: Option<String>,
    pub output_pattern: Option<String>,
    pub output_tuples: bool,
    pub output_unit: Option<usize>,
    pub output_raw_unit: Option<u64>,
    pub output_offset: u64,
    pub output_post_gap: u64,
    pub output_gap: u64,
    pub output_skip_bits: u64,
    pub output_reverse_bytes: bool,
    pub output_reverse_unit: bool,
    pub output_integers: bool,
    pub output_hex: bool,
    pub output_bits: bool,
    pub output_json: bool,
    pub output_csv: bool,
    pub csv_header: Option<String>,
    pub output_visual: bool,
    pub demux: Vec<String>,
    pub demux_files: Option<String>,
    pub input_repeat: usize,
    pub quiet: bool,
    pub merge_file: Option<String>,
    pub merge_files: Vec<String>,
    pub merge_unit: Option<usize>,
    pub merge_skip_bits: u64,
    pub merge_skip_units: u64,
    pub merge_copy_first: u64,
    pub merge_gap: u64,
    pub merge_assert_aligned: bool,
    pub merge_drop_partial_eof: bool,
    pub merge_use_seek: bool,
    pub merge_use_mmap: bool,
    pub merge_reverse_bytes: bool,
    pub merge_reverse_unit: bool,
    pub raw_args: Vec<String>,
    pub json_fields: Option<Vec<String>>,
    pub json_object: bool,
    pub preset: Option<String>,
    pub list_presets: bool,
    pub explain_pattern: Option<String>,
    pub probe: Option<String>,
    pub probe_units: bool,
    pub probe_keys: Option<String>,
    pub probe_field: Option<usize>,
    pub probe_visual: bool,
    pub inline_manipulators: Vec<String>,
    pub mcp: bool,
    pub overwrite: bool,
    pub merge_specs: Vec<crate::stream_pattern::StreamSpec>,
}

fn check_exclusive(msg: &str, flags: &[bool]) -> Result<(), BddError> {
    let count = flags.iter().filter(|&&b| b).count();
    if count > 1 {
        return Err(BddError::CliError(msg.to_string()));
    }
    Ok(())
}

/// Parses a size or count string supporting standard binary and decimal suffixes.
///
fn parse_single_factor(s: &str, option_name: &str) -> Result<(u64, bool), BddError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(BddError::CliError(format!(
            "Empty factor in value for {}",
            option_name
        )));
    }

    if s.starts_with('-') {
        return Err(BddError::CliError(format!(
            "{} value must be a positive integer",
            option_name
        )));
    }

    if let Some((base_str, exp_str)) = s.split_once('^') {
        let (base, is_byte1) = parse_single_factor(base_str, option_name)?;
        let (exp, is_byte2) = parse_single_factor(exp_str, option_name)?;
        let val = if exp >= 64 && base > 1 {
            u64::MAX
        } else {
            base.saturating_pow(exp as u32)
        };
        return Ok((val, is_byte1 || is_byte2));
    }

    // Support hex literal if starts with 0x / 0X
    if s.starts_with("0x") || s.starts_with("0X") {
        let hex_str = &s[2..];
        let val = u64::from_str_radix(hex_str, 16).map_err(|_| {
            BddError::CliError(format!("Invalid hex number for {}: '{}'", option_name, s))
        })?;
        return Ok((val, false));
    }

    // Otherwise split into number part and suffix
    let end_of_num = s
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(s.len());
    let (num_str, suffix_raw) = s.split_at(end_of_num);
    let suffix = suffix_raw.trim();

    if num_str.is_empty() {
        return Err(BddError::CliError(format!(
            "Invalid numeric value for {}: '{}'",
            option_name, s
        )));
    }

    let lower_suffix = suffix.to_ascii_lowercase();
    let (scale, is_byte_unit) = match lower_suffix.as_str() {
        "" => (1u64, false),
        "b" if suffix == "B" => (1u64, true),
        "b" | "bit" | "bits" => (1u64, false),
        "byte" | "bytes" => (1u64, true),
        // Binary multiples (powers of 1024)
        "k" | "ki" | "kib" => (
            1024u64,
            lower_suffix.ends_with("ib") || suffix.ends_with('B'),
        ),
        "m" | "mi" | "mib" => (
            1024u64.pow(2),
            lower_suffix.ends_with("ib") || suffix.ends_with('B'),
        ),
        "g" | "gi" | "gib" => (
            1024u64.pow(3),
            lower_suffix.ends_with("ib") || suffix.ends_with('B'),
        ),
        "t" | "ti" | "tib" => (
            1024u64.pow(4),
            lower_suffix.ends_with("ib") || suffix.ends_with('B'),
        ),
        "p" | "pi" | "pib" => (
            1024u64.pow(5),
            lower_suffix.ends_with("ib") || suffix.ends_with('B'),
        ),
        "e" | "ei" | "eib" => (
            1024u64.pow(6),
            lower_suffix.ends_with("ib") || suffix.ends_with('B'),
        ),
        // Decimal multiples (powers of 1000)
        "kb" => (1000u64, suffix.ends_with('B')),
        "mb" => (1000u64.pow(2), suffix.ends_with('B')),
        "gb" => (1000u64.pow(3), suffix.ends_with('B')),
        "tb" => (1000u64.pow(4), suffix.ends_with('B')),
        "pb" => (1000u64.pow(5), suffix.ends_with('B')),
        "eb" => (1000u64.pow(6), suffix.ends_with('B')),
        other => {
            return Err(BddError::CliError(format!(
                "Unknown unit suffix '{}' for {}",
                other, option_name
            )));
        }
    };

    if num_str.contains('.') {
        let f: f64 = num_str.parse().map_err(|_| {
            BddError::CliError(format!("Invalid number for {}: '{}'", option_name, s))
        })?;
        if f < 0.0 {
            return Err(BddError::CliError(format!(
                "{} value must be a positive integer",
                option_name
            )));
        }
        let total = f * (scale as f64);
        if total > u64::MAX as f64 {
            return Err(BddError::CliError(format!(
                "Value for {} exceeds 64-bit integer limit: '{}'",
                option_name, s
            )));
        }
        Ok((total as u64, is_byte_unit))
    } else {
        let n: u64 = num_str.parse().map_err(|_| {
            BddError::CliError(format!("Invalid number for {}: '{}'", option_name, s))
        })?;
        let total = n.checked_mul(scale).ok_or_else(|| {
            BddError::CliError(format!(
                "Value for {} exceeds 64-bit integer limit: '{}'",
                option_name, s
            ))
        })?;
        Ok((total, is_byte_unit))
    }
}

/// Parses a size or count string supporting standard binary and decimal suffixes
/// as well as multiplication expressions (e.g. `1000000*24` or `1920x1080*3`).
///
/// If `is_bit_option` is true (e.g. `--input-skip-bits`), byte suffixes (`B`, `Bytes`, `KiB`, `GiB`, etc.)
/// are multiplied by 8 to convert bytes to bits.
pub fn parse_size_with_suffix(
    s: &str,
    option_name: &str,
    is_bit_option: bool,
) -> Result<u64, BddError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(BddError::CliError(format!(
            "Empty value for {}",
            option_name
        )));
    }

    if s.starts_with('-') {
        return Err(BddError::CliError(format!(
            "{} value must be a positive integer",
            option_name
        )));
    }

    // Split by '*' first
    let mut factors = Vec::new();
    for star_part in s.split('*') {
        let trimmed = star_part.trim();
        // If not a hex literal and contains 'x' or 'X', split by 'x' / 'X'
        if !trimmed.starts_with("0x")
            && !trimmed.starts_with("0X")
            && (trimmed.contains('x') || trimmed.contains('X'))
        {
            for x_part in trimmed.split(['x', 'X']) {
                factors.push(x_part.trim());
            }
        } else {
            factors.push(trimmed);
        }
    }

    if factors.is_empty() {
        return Err(BddError::CliError(format!(
            "Empty value for {}",
            option_name
        )));
    }

    let mut total = 1u64;
    let mut any_byte_unit = false;

    for factor in factors {
        if factor.is_empty() {
            return Err(BddError::CliError(format!(
                "Empty factor in multiplication expression for {}: '{}'",
                option_name, s
            )));
        }
        let (val, is_byte) = parse_single_factor(factor, option_name)?;
        any_byte_unit |= is_byte;
        total = total.checked_mul(val).ok_or_else(|| {
            BddError::CliError(format!(
                "Value for {} exceeds 64-bit integer limit: '{}'",
                option_name, s
            ))
        })?;
    }

    if is_bit_option && any_byte_unit {
        total = total.checked_mul(8).ok_or_else(|| {
            BddError::CliError(format!(
                "Value for {} in bits exceeds 64-bit integer limit: '{}'",
                option_name, s
            ))
        })?;
    }

    Ok(total)
}

fn parse_number_argument(
    val: Option<&str>,
    option_name: &str,
    default: Option<u64>,
    is_bit_option: bool,
) -> Result<Option<u64>, BddError> {
    match val {
        None => Ok(default),
        Some(s) => {
            let s_trim = s.trim();
            if s_trim.starts_with('-') {
                crate::diag::warn(format!("{} value must be a positive integer", option_name));
                match default {
                    Some(d) => {
                        crate::diag::warn(format!("Assumed {}={}", option_name, d));
                        Ok(Some(d))
                    }
                    None => {
                        crate::diag::warn(format!("Assumed {}=None", option_name));
                        Ok(None)
                    }
                }
            } else {
                let parsed = parse_size_with_suffix(s_trim, option_name, is_bit_option)?;
                Ok(Some(parsed))
            }
        }
    }
}

/// Validate CLI flags against legacy exclusivity rules and calculate effective options.
pub fn validate_and_process(mut cli: Cli) -> Result<ValidatedConfig, BddError> {
    crate::diag::set_quiet(cli.quiet);

    // 1. Process positional argument:
    // bdd [STREAM_PATTERN]
    let mut stream_pat = cli.stream_pattern.take();

    // Disambiguation: if stream_pat does not look like a stream pattern, but looks like a tuple pattern (e.g. 4U4U)
    if let Some(ref sp) = stream_pat {
        if !crate::stream_pattern::is_stream_io_pattern(sp) {
            let pat = stream_pat.take().unwrap();
            // When reading text tuples (--input-tuples), the input is already divided into fields by commas.
            // If a single positional tuple pattern is provided with --input-tuples, it specifies the output packing pattern.
            if cli.input_tuples && cli.output_pattern.is_none() {
                cli.output_pattern = Some(pat);
            } else if cli.input_pattern.is_none() {
                cli.input_pattern = Some(pat);
            }
        }
    }
    let cli_explicit_input_unit = cli.input_unit.is_some();
    let cli_explicit_output_unit = cli.output_unit.is_some();
    let mut inline_manipulators = Vec::new();

    let mut merge_specs = Vec::new();

    if let Some(ref sp) = stream_pat {
        let parsed = crate::stream_pattern::parse_stream_io_pattern(sp)?;
        inline_manipulators = parsed.manipulators;
        if parsed.overwrite {
            cli.overwrite = true;
        }
        merge_specs = parsed.merge_specs.clone();
        if let Some(ref inp) = parsed.input {
            if cli.input_skip_bits.is_none() && inp.skip.is_some() {
                cli.input_skip_bits = inp.skip.map(|v| v.to_string());
            }
            if cli.input_raw_unit.is_none() && inp.raw_unit.is_some() {
                cli.input_raw_unit = inp.raw_unit.map(|v| v.to_string());
            }
            if cli.input_offset.is_none() && inp.offset.is_some() {
                cli.input_offset = inp.offset.map(|v| v.to_string());
            }
            if cli.input_unit.is_none() && inp.unit_size.is_some() {
                cli.input_unit = inp.unit_size.map(|v| v.to_string());
            }
            if cli.input_pattern.is_none() && inp.pattern.is_some() {
                cli.input_pattern = inp.pattern.clone();
            }
            if cli.input_gap.is_none() && inp.gap.is_some() {
                cli.input_gap = inp.gap.map(|v| v.to_string());
            }
        }
        if let Some(ref out) = parsed.output {
            if cli.output_skip_bits.is_none() && out.skip.is_some() {
                cli.output_skip_bits = out.skip.map(|v| v.to_string());
            }
            if cli.output_raw_unit.is_none() && out.raw_unit.is_some() {
                cli.output_raw_unit = out.raw_unit.map(|v| v.to_string());
            }
            if cli.output_offset.is_none() && out.offset.is_some() {
                cli.output_offset = out.offset.map(|v| v.to_string());
            }
            if cli.output_unit.is_none() && out.unit_size.is_some() {
                cli.output_unit = out.unit_size.map(|v| v.to_string());
            }
            if cli.output_pattern.is_none() && out.pattern.is_some() {
                cli.output_pattern = out.pattern.clone();
            }
            if cli.output_gap.is_none() && out.gap.is_some() {
                cli.output_gap = out.gap.map(|v| v.to_string());
            }
        }
        if let Some(ref src) = parsed.source {
            match src.as_str() {
                "stdin" => cli.input_file = "-".to_string(),
                "zeros" => cli.input_zeros = true,
                "ones" => cli.input_ones = true,
                "rand" => cli.input_random = true,
                "counter" => cli.input_counter = true,
                s if s.starts_with("counter(") && s.ends_with(')') => {
                    cli.input_counter = true;
                }
                "tuples" => cli.input_tuples = true,
                s if s.starts_with("file(") && s.ends_with(')') => {
                    let path = s[5..s.len() - 1]
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"');
                    cli.input_file = path.to_string();
                }
                other => {
                    let path = other.trim().trim_matches('\'').trim_matches('"');
                    if !path.is_empty() {
                        cli.input_file = path.to_string();
                    }
                }
            }
        }
        if let Some(ref sink) = parsed.sink {
            match sink.as_str() {
                "stdout" => cli.output_file = "-".to_string(),
                "hex" => cli.output_hex = true,
                "bits" => cli.output_bits = true,
                "json" => cli.output_json = true,
                s if s.starts_with("json:") => {
                    cli.output_json = true;
                    if s == "json:object" {
                        cli.json_object = true;
                    }
                }
                "csv" => cli.output_csv = true,
                "tuples" => cli.output_tuples = true,
                "visual" => cli.output_visual = true,
                "integers" => cli.output_integers = true,
                "raw" | "bin" => {}
                s if s.starts_with("file(") && s.ends_with(')') => {
                    let path = s[5..s.len() - 1]
                        .trim()
                        .trim_matches('\'')
                        .trim_matches('"');
                    cli.output_file = path.to_string();
                }
                other => {
                    let path = other.trim().trim_matches('\'').trim_matches('"');
                    if !path.is_empty() {
                        cli.output_file = path.to_string();
                    }
                }
            }
        }
        if cli.overwrite {
            if cli.output_raw_unit.is_none() {
                cli.output_raw_unit = cli.input_raw_unit.clone();
            }
            if cli.output_offset.is_none() {
                cli.output_offset = cli.input_offset.clone();
            }
            if cli.output_unit.is_none() {
                cli.output_unit = cli.input_unit.clone();
            }
            if cli.output_gap.is_none() {
                cli.output_gap = cli.input_gap.clone();
            }
            if cli.output_skip_bits.is_none() {
                cli.output_skip_bits = cli.input_skip_bits.clone();
            }
        }
    }

    if let Some(ref path) = cli.presets_file {
        crate::preset::set_custom_presets_path(path.clone());
    }

    if let Some(ref preset_name) = cli.preset {
        if let Some(preset) = crate::preset::find_preset(preset_name) {
            if cli.input_pattern.is_none() {
                cli.input_pattern = Some(preset.pattern.to_string());
            }
            if preset.little_endian {
                cli.input_little_endian = true;
            }
            if cli.count.is_none() && preset.default_count.is_some() {
                cli.count = preset.default_count.map(|c| c.to_string());
            }
            let has_output_format = cli.output_tuples
                || cli.output_integers
                || cli.output_hex
                || cli.output_bits
                || cli.output_json
                || cli.output_csv
                || cli.output_visual;
            if !has_output_format {
                cli.output_json = true;
            }
        } else {
            #[cfg(not(feature = "small-floats"))]
            {
                let normalized = preset_name.to_lowercase().replace('_', "-");
                if ["nvfp4", "fp6-e3m2", "fp8-e4m3", "fp8-e5m2", "bf16", "fp16"]
                    .contains(&normalized.as_str())
                {
                    return Err(BddError::CliError(format!(
                        "Preset '{}' requires the 'small-floats' feature to be enabled. Recompile with --features small-floats.",
                        preset_name
                    )));
                }
            }
            return Err(BddError::CliError(format!(
                "Unknown preset '{}'. Use --list-presets to see available presets.",
                preset_name
            )));
        }
    }

    check_exclusive(
        "Only one of the following is allowed: --input-zeros, --input-ones, --input-random, --input-counter, --input-integers, --input-tuples",
        &[
            cli.input_zeros,
            cli.input_ones,
            cli.input_random,
            cli.input_counter,
            cli.input_integers,
            cli.input_tuples,
        ],
    )?;

    let has_special_stream =
        cli.input_zeros || cli.input_ones || cli.input_random || cli.input_counter;

    if cli.input_file != "-" && has_special_stream {
        return Err(BddError::CliError(
            "--input-file can not be used with --input-zeros, --input-ones, --input-random or --input-counter".to_string(),
        ));
    }

    let skip = parse_number_argument(cli.skip.as_deref(), "--skip", Some(0), false)?.unwrap_or(0);
    let count_is_cycle = cli
        .count
        .as_deref()
        .map(|s| {
            let trimmed = s.trim().to_ascii_lowercase();
            trimmed == "cycle" || trimmed == "full-range" || trimmed == "full_range"
        })
        .unwrap_or(false);

    let count = if count_is_cycle {
        None
    } else {
        parse_number_argument(cli.count.as_deref(), "--count", None, false)?
    };

    check_exclusive(
        "Only one of the following: --output-tuples, --output-integers, --output-hex, --output-bits, --output-json, --output-csv, --output-visual",
        &[
            cli.output_tuples,
            cli.output_integers,
            cli.output_hex,
            cli.output_bits,
            cli.output_json,
            cli.output_csv,
            cli.output_visual,
        ],
    )?;

    let pattern_input_unit = if let Some(ref p) = cli.input_pattern {
        let items = crate::pattern::parse_input_pattern(p)?;
        Some(items.iter().map(|it| it.bits).sum::<usize>())
    } else {
        None
    };

    let pattern_output_unit = if let Some(ref p) = cli.output_pattern {
        let items = crate::pattern::parse_output_pattern(p)?;
        Some(items.iter().map(|it| it.bits).sum::<usize>())
    } else {
        None
    };

    if cli.input_pattern.is_some() && cli.input_unit.is_some() {
        if cli_explicit_input_unit {
            crate::diag::warn("--input-pattern overwrites --input-unit");
        }
        cli.input_unit = None;
    }
    if cli.output_pattern.is_some() && cli.output_unit.is_some() {
        if cli_explicit_output_unit {
            crate::diag::warn("--output-pattern overwrites --output-unit");
        }
        cli.output_unit = None;
    }

    let mut all_merge_files: Vec<String> = Vec::new();
    for f in &cli.merge_file {
        for part in f.split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                all_merge_files.push(trimmed.to_string());
            }
        }
    }
    for f in &cli.merge_files {
        for part in f.split(|c: char| c == ',' || c.is_whitespace()) {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                all_merge_files.push(trimmed.to_string());
            }
        }
    }

    let merge_specified = !all_merge_files.is_empty();
    if !merge_specified {
        let has_merge_opt = cli.merge_unit.is_some()
            || cli.merge_raw_unit.is_some()
            || cli.merge_skip_bits.is_some()
            || cli.merge_skip_units.is_some()
            || cli.merge_copy_first.is_some()
            || cli.merge_gap.is_some()
            || cli.merge_offset.is_some()
            || cli.merge_assert_aligned
            || cli.merge_drop_partial_eof
            || cli.merge_little_endian
            || cli.merge_reverse_bytes
            || cli.merge_reverse_unit;
        if has_merge_opt {
            return Err(BddError::CliError(
                "Merge options can be used only if --merge-file option is set".to_string(),
            ));
        }
    }

    if cli.input_offset.is_some() && cli.input_raw_unit.is_none() {
        return Err(BddError::CliError(
            "--input-offset requires --input-raw-unit to be specified".to_string(),
        ));
    }
    if cli.merge_offset.is_some() && cli.merge_raw_unit.is_none() {
        return Err(BddError::CliError(
            "--merge-offset requires --merge-raw-unit to be specified".to_string(),
        ));
    }

    let raw_unit = parse_number_argument(
        cli.input_raw_unit.as_deref(),
        "--input-raw-unit",
        None,
        false,
    )?;
    let input_offset = parse_number_argument(
        cli.input_offset.as_deref(),
        "--input-offset",
        Some(0),
        false,
    )?
    .unwrap_or(0);
    let input_gap_raw =
        parse_number_argument(cli.input_gap.as_deref(), "--input-gap", Some(0), false)?
            .unwrap_or(0);
    let input_skip_bits_raw = parse_number_argument(
        cli.input_skip_bits.as_deref(),
        "--input-skip-bits",
        Some(0),
        true,
    )?
    .unwrap_or(0);
    let input_skip_units_raw = parse_number_argument(
        cli.input_skip_units.as_deref(),
        "--input-skip-units",
        Some(0),
        false,
    )?
    .unwrap_or(0);
    let input_unit = parse_number_argument(cli.input_unit.as_deref(), "--input-unit", None, false)?
        .map(|v| v as usize)
        .or(pattern_input_unit);

    let (input_skip_bits, input_skip_units, input_gap, resolved_input_unit) =
        if let Some(r) = raw_unit {
            if r == 0 {
                return Err(BddError::CliError(
                    "--input-raw-unit must be greater than 0".to_string(),
                ));
            }
            let u = if let Some(unit) = input_unit {
                if unit == 0 {
                    return Err(BddError::CliError(
                        "--input-unit must be greater than 0".to_string(),
                    ));
                }
                if input_offset + (unit as u64) > r {
                    return Err(BddError::CliError(format!(
                    "--input-offset ({}) + input unit/pattern ({}) exceeds --input-raw-unit ({})",
                    input_offset, unit, r
                )));
                }
                unit
            } else {
                if input_offset >= r {
                    return Err(BddError::CliError(format!(
                        "--input-offset ({}) must be less than --input-raw-unit ({})",
                        input_offset, r
                    )));
                }
                (r - input_offset) as usize
            };
            let skip_stride = r + input_gap_raw;
            let (skip_bits, gap) = if cli.overwrite {
                (
                    input_skip_bits_raw + (input_skip_units_raw * skip_stride),
                    input_gap_raw,
                )
            } else {
                (
                    input_skip_bits_raw + (input_skip_units_raw * skip_stride) + input_offset,
                    r - (u as u64) + input_gap_raw,
                )
            };
            (skip_bits, 0, gap, Some(u))
        } else {
            (
                input_skip_bits_raw,
                input_skip_units_raw,
                input_gap_raw,
                input_unit,
            )
        };

    let count = if count_is_cycle {
        let u = resolved_input_unit.or(pattern_input_unit).unwrap_or(8);
        Some(if u >= 64 { u64::MAX } else { 1u64 << u })
    } else {
        count
    };

    let merge_raw_unit = parse_number_argument(
        cli.merge_raw_unit.as_deref(),
        "--merge-raw-unit",
        None,
        false,
    )?;
    let merge_offset = parse_number_argument(
        cli.merge_offset.as_deref(),
        "--merge-offset",
        Some(0),
        false,
    )?
    .unwrap_or(0);
    let merge_gap_raw =
        parse_number_argument(cli.merge_gap.as_deref(), "--merge-gap", Some(0), false)?
            .unwrap_or(0);
    let merge_skip_bits_raw = parse_number_argument(
        cli.merge_skip_bits.as_deref(),
        "--merge-skip-bits",
        Some(0),
        true,
    )?
    .unwrap_or(0);
    let merge_skip_units_raw = parse_number_argument(
        cli.merge_skip_units.as_deref(),
        "--merge-skip-units",
        Some(0),
        false,
    )?
    .unwrap_or(0);
    let merge_copy_first = parse_number_argument(
        cli.merge_copy_first.as_deref(),
        "--merge-copy-first",
        Some(0),
        false,
    )?
    .unwrap_or(0);
    let merge_unit = parse_number_argument(cli.merge_unit.as_deref(), "--merge-unit", None, false)?
        .map(|v| v as usize);

    let (merge_skip_bits, merge_skip_units, merge_gap, resolved_merge_unit) = if let Some(r) =
        merge_raw_unit
    {
        if r == 0 {
            return Err(BddError::CliError(
                "--merge-raw-unit must be greater than 0".to_string(),
            ));
        }
        let u = if let Some(unit) = merge_unit {
            if unit == 0 {
                return Err(BddError::CliError(
                    "--merge-unit must be greater than 0".to_string(),
                ));
            }
            if merge_offset + (unit as u64) > r {
                return Err(BddError::CliError(format!(
                    "--merge-offset ({}) + --merge-unit ({}) exceeds --merge-raw-unit ({})",
                    merge_offset, unit, r
                )));
            }
            unit
        } else {
            if merge_offset >= r {
                return Err(BddError::CliError(format!(
                    "--merge-offset ({}) must be less than --merge-raw-unit ({})",
                    merge_offset, r
                )));
            }
            (r - merge_offset) as usize
        };
        let skip_stride = r + merge_gap_raw;
        let skip_bits = merge_skip_bits_raw + (merge_skip_units_raw * skip_stride) + merge_offset;
        let gap = r - (u as u64) + merge_gap_raw;
        (skip_bits, 0, gap, Some(u))
    } else {
        (
            merge_skip_bits_raw,
            merge_skip_units_raw,
            merge_gap_raw,
            merge_unit,
        )
    };

    let resolved_output_unit =
        parse_number_argument(cli.output_unit.as_deref(), "--output-unit", None, false)?
            .map(|v| v as usize)
            .or(pattern_output_unit);

    let output_raw_unit = parse_number_argument(
        cli.output_raw_unit.as_deref(),
        "--output-raw-unit",
        None,
        false,
    )?;
    let output_offset = parse_number_argument(
        cli.output_offset.as_deref(),
        "--output-offset",
        Some(0),
        false,
    )?
    .unwrap_or(0);
    let output_gap =
        parse_number_argument(cli.output_gap.as_deref(), "--output-gap", Some(0), false)?
            .unwrap_or(0);
    let output_skip_bits = parse_number_argument(
        cli.output_skip_bits.as_deref(),
        "--output-skip-bits",
        Some(0),
        true,
    )?
    .unwrap_or(0);

    if output_offset > 0 && output_raw_unit.is_none() {
        return Err(BddError::CliError(
            "--output-offset requires --output-raw-unit to be specified".to_string(),
        ));
    }

    let output_post_gap = if let Some(r) = output_raw_unit {
        if r == 0 {
            return Err(BddError::CliError(
                "--output-raw-unit must be greater than 0".to_string(),
            ));
        }
        let u = resolved_output_unit.unwrap_or(8) as u64;
        if output_offset + u > r {
            return Err(BddError::CliError(format!(
                "--output-offset ({}) + output unit/pattern ({}) exceeds --output-raw-unit ({})",
                output_offset, u, r
            )));
        }
        r - output_offset - u
    } else {
        0
    };

    if cli.input_little_endian {
        if cli.input_reverse_bytes || cli.input_reverse_unit {
            crate::diag::warn(
                "--input-little-endian overwrites --input-reverse-bytes and --input-reverse-unit",
            );
        }
        cli.input_reverse_bytes = true;
        cli.input_reverse_unit = true;
    }

    if cli.output_little_endian {
        if cli.output_reverse_bytes || cli.output_reverse_unit {
            crate::diag::warn(
                "--output-little-endian overwrites --output-reverse-bytes and --output-reverse-unit",
            );
        }
        cli.output_reverse_bytes = true;
        cli.output_reverse_unit = true;
    }

    if cli.merge_little_endian {
        if cli.merge_reverse_bytes || cli.merge_reverse_unit {
            crate::diag::warn(
                "--merge-little-endian overwrites --merge-reverse-bytes and --merge-reverse-unit",
            );
        }
        cli.merge_reverse_bytes = true;
        cli.merge_reverse_unit = true;
    }

    if cli.input_file == "-" && cli.input_use_seek {
        crate::diag::warn("Standard input does not allow seek.--input-use-seek disabled");
        cli.input_use_seek = false;
    }

    // Auto-seek by default on regular files / seekable descriptors, unless explicitly disabled
    let use_seek = !cli.no_seek;
    let input_use_seek = use_seek;
    let merge_use_seek = use_seek;

    // Memory-mapped I/O by default on regular files when supported, unless explicitly disabled
    let use_mmap = if cli.no_mmap {
        false
    } else {
        cli.mmap || cfg!(feature = "mmap")
    };
    let input_use_mmap = if cli.input_file == "-" {
        false
    } else {
        use_mmap
    };
    let merge_use_mmap = use_mmap;

    #[cfg(not(feature = "mmap"))]
    if cli.mmap {
        crate::diag::warn("Memory-mapped I/O was requested, but bdd was compiled without the 'mmap' feature. Falling back to standard buffered I/O.");
    }

    let input_repeat = parse_number_argument(
        cli.input_repeat.as_deref(),
        "--input-repeat",
        Some(1),
        false,
    )?
    .unwrap_or(1) as usize;

    Ok(ValidatedConfig {
        input_file: cli.input_file,
        output_file: cli.output_file,
        input_unit: resolved_input_unit,
        input_raw_unit: raw_unit,
        input_offset,
        input_skip_bits,
        input_skip_units,
        input_gap,
        input_assert_aligned: cli.input_assert_aligned,
        input_drop_partial_eof: cli.input_drop_partial_eof,
        input_use_seek,
        input_use_mmap,
        input_reverse_bytes: cli.input_reverse_bytes,
        input_reverse_unit: cli.input_reverse_unit,
        input_zeros: cli.input_zeros,
        input_ones: cli.input_ones,
        input_random: cli.input_random,
        input_counter: cli.input_counter,
        input_integers: cli.input_integers,
        input_pattern: cli.input_pattern,
        input_tuples: cli.input_tuples,
        skip,
        count,
        rearrange: cli.rearrange,
        clamp: cli.clamp,
        round: cli.round.clone(),
        cut_maxint: cli.round,
        remove_right: cli.remove_right,
        shift_right: cli.shift_right,
        shift_left: cli.shift_left,
        xor: cli.xor,
        and: cli.and,
        or: cli.or,
        not: cli.not,
        abs: cli.abs,
        sign: cli.sign,
        add: cli.add,
        sub: cli.sub,
        mul: cli.mul,
        div: cli.div,
        r#mod: cli.r#mod,
        filter: cli.filter,
        set: cli.set,
        output_pattern: cli.output_pattern,
        output_tuples: cli.output_tuples,
        output_unit: resolved_output_unit,
        output_raw_unit,
        output_offset,
        output_post_gap,
        output_gap,
        output_skip_bits,
        output_reverse_bytes: cli.output_reverse_bytes,
        output_reverse_unit: cli.output_reverse_unit,
        output_integers: cli.output_integers,
        output_hex: cli.output_hex,
        output_bits: cli.output_bits,
        output_json: cli.output_json,
        output_csv: cli.output_csv,
        csv_header: cli.csv_header,
        output_visual: cli.output_visual,
        demux: cli.demux,
        demux_files: cli.demux_files,
        input_repeat,
        quiet: cli.quiet,
        merge_file: all_merge_files.first().cloned(),
        merge_files: all_merge_files,
        merge_unit: resolved_merge_unit,
        merge_skip_bits,
        merge_skip_units,
        merge_copy_first,
        merge_gap,
        merge_assert_aligned: cli.merge_assert_aligned,
        merge_drop_partial_eof: cli.merge_drop_partial_eof,
        merge_use_seek,
        merge_use_mmap,
        merge_reverse_bytes: cli.merge_reverse_bytes,
        merge_reverse_unit: cli.merge_reverse_unit,
        raw_args: Vec::new(),
        json_fields: cli.json_fields.as_deref().map(|s| {
            s.split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        }),
        json_object: cli.json_object,
        preset: cli.preset,
        list_presets: cli.list_presets,
        explain_pattern: cli.explain_pattern,
        probe: cli.probe,
        probe_units: cli.probe_units || cli.probe_keys.is_some() || cli.probe_field.is_some(),
        probe_keys: if cli.probe_keys.is_some() {
            cli.probe_keys
        } else if cli.probe_units || cli.probe_field.is_some() {
            Some("256".to_string())
        } else {
            None
        },
        probe_field: cli.probe_field,
        probe_visual: cli.probe_visual,
        inline_manipulators,
        mcp: cli.mcp,
        overwrite: cli.overwrite,
        merge_specs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_cli() {
        let cli = Cli::parse_from(["bdd", "--input-zeros", "--count", "10"]);
        let config = validate_and_process(cli).unwrap();
        assert!(config.input_zeros);
        assert_eq!(config.count, Some(10));
    }

    #[test]
    fn test_exclusive_validation() {
        let cli = Cli::parse_from(["bdd", "--input-zeros", "--input-ones"]);
        assert!(validate_and_process(cli).is_err());
    }

    #[test]
    fn test_seek_defaults_and_options() {
        // Default: seeking is enabled for both input and merge
        let cli_default = Cli::parse_from(["bdd", "--input-zeros", "--count=1"]);
        let conf_default = validate_and_process(cli_default).unwrap();
        assert!(conf_default.input_use_seek);
        assert!(conf_default.merge_use_seek);

        // Global --no-seek disables seeking
        let cli_no_seek = Cli::parse_from(["bdd", "--no-seek", "--input-zeros", "--count=1"]);
        let conf_no_seek = validate_and_process(cli_no_seek).unwrap();
        assert!(!conf_no_seek.input_use_seek);
        assert!(!conf_no_seek.merge_use_seek);

        // Alias --do-not-seek
        let cli_do_not_seek =
            Cli::parse_from(["bdd", "--do-not-seek", "--input-zeros", "--count=1"]);
        let conf_do_not_seek = validate_and_process(cli_do_not_seek).unwrap();
        assert!(!conf_do_not_seek.input_use_seek);
        assert!(!conf_do_not_seek.merge_use_seek);

        // Aliases --input-no-seek and --merge-no-seek
        let cli_input_no_seek =
            Cli::parse_from(["bdd", "--input-no-seek", "--input-zeros", "--count=1"]);
        let conf_input_no_seek = validate_and_process(cli_input_no_seek).unwrap();
        assert!(!conf_input_no_seek.input_use_seek);
        assert!(!conf_input_no_seek.merge_use_seek);

        let cli_merge_no_seek =
            Cli::parse_from(["bdd", "--merge-no-seek", "--input-zeros", "--count=1"]);
        let conf_merge_no_seek = validate_and_process(cli_merge_no_seek).unwrap();
        assert!(!conf_merge_no_seek.input_use_seek);
        assert!(!conf_merge_no_seek.merge_use_seek);
    }

    #[test]
    fn test_little_endian_and_reversal_aliases() {
        let cli = Cli::parse_from(["bdd", "--little-endian", "--input-zeros", "--count=1"]);
        assert!(cli.input_little_endian);
        let conf = validate_and_process(cli).unwrap();
        assert!(conf.input_reverse_bytes);
        assert!(conf.input_reverse_unit);

        let cli_rev = Cli::parse_from([
            "bdd",
            "--reverse-input-bytes",
            "--reverse-input-units",
            "--reverse-output-bytes",
            "--reverse-output-units",
            "--input-zeros",
            "--count=1",
        ]);
        assert!(cli_rev.input_reverse_bytes);
        assert!(cli_rev.input_reverse_unit);
        assert!(cli_rev.output_reverse_bytes);
        assert!(cli_rev.output_reverse_unit);
    }

    #[test]
    fn test_raw_unit_and_offset() {
        // Extraction A: Bits 3 and 4 of every 8-bit byte (offset 2, unit 2)
        let cli_a = Cli::parse_from([
            "bdd",
            "--input-zeros",
            "--count=1",
            "--input-raw-unit=8",
            "--input-offset=2",
            "--input-unit=2",
        ]);
        let conf_a = validate_and_process(cli_a).unwrap();
        assert_eq!(conf_a.input_unit, Some(2));
        assert_eq!(conf_a.input_skip_bits, 2);
        assert_eq!(conf_a.input_gap, 6); // 8 - 2 + 0 = 6

        // Extraction B: Bit 5 of every 8-bit byte (offset 4, unit 1)
        let cli_b = Cli::parse_from([
            "bdd",
            "--input-zeros",
            "--count=1",
            "--input-raw-unit=8",
            "--input-offset=4",
            "--input-unit=1",
        ]);
        let conf_b = validate_and_process(cli_b).unwrap();
        assert_eq!(conf_b.input_unit, Some(1));
        assert_eq!(conf_b.input_skip_bits, 4);
        assert_eq!(conf_b.input_gap, 7); // 8 - 1 + 0 = 7

        // Raw unit with inter-raw-unit gap: 16-bit packet, 8-bit inter-packet gap
        let cli_c = Cli::parse_from([
            "bdd",
            "--input-zeros",
            "--count=1",
            "--input-raw-unit=16",
            "--input-offset=2",
            "--input-unit=4",
            "--input-gap=8",
        ]);
        let conf_c = validate_and_process(cli_c).unwrap();
        assert_eq!(conf_c.input_unit, Some(4));
        assert_eq!(conf_c.input_skip_bits, 2);
        assert_eq!(conf_c.input_gap, 20); // 16 - 4 + 8 = 20

        // Unit omitted: defaults to remaining bits in raw unit (8 - 2 = 6)
        let cli_d = Cli::parse_from([
            "bdd",
            "--input-zeros",
            "--count=1",
            "--input-raw-unit=8",
            "--input-offset=2",
        ]);
        let conf_d = validate_and_process(cli_d).unwrap();
        assert_eq!(conf_d.input_unit, Some(6));
        assert_eq!(conf_d.input_skip_bits, 2);
        assert_eq!(conf_d.input_gap, 2); // 8 - 6 + 0 = 2

        // Error when offset + unit exceeds raw-unit
        let cli_err = Cli::parse_from([
            "bdd",
            "--input-zeros",
            "--count=1",
            "--input-raw-unit=8",
            "--input-offset=6",
            "--input-unit=4",
        ]);
        assert!(validate_and_process(cli_err).is_err());
    }

    #[test]
    fn test_offset_without_raw_unit_fails() {
        let cli = Cli::parse_from(["bdd", "--input-zeros", "--count=1", "--input-offset=2"]);
        let err = validate_and_process(cli);
        assert!(err.is_err());
        assert!(err
            .unwrap_err()
            .to_string()
            .contains("--input-offset requires --input-raw-unit"));

        let cli_merge = Cli::parse_from([
            "bdd",
            "--input-zeros",
            "--count=1",
            "--merge-file",
            "/dev/null",
            "--merge-offset=2",
        ]);
        let err_merge = validate_and_process(cli_merge);
        assert!(err_merge.is_err());
        assert!(err_merge
            .unwrap_err()
            .to_string()
            .contains("--merge-offset requires --merge-raw-unit"));
    }

    #[test]
    fn test_size_suffixes() {
        // Test helper function
        assert_eq!(
            parse_size_with_suffix("10G", "--input-skip-bits", true).unwrap(),
            10 * 1024 * 1024 * 1024
        );
        assert_eq!(
            parse_size_with_suffix("1GiB", "--input-skip-bits", true).unwrap(),
            1024 * 1024 * 1024 * 8
        );
        assert_eq!(
            parse_size_with_suffix("1GB", "--input-skip-bits", true).unwrap(),
            1000 * 1000 * 1000 * 8
        );
        assert_eq!(
            parse_size_with_suffix("10M", "--skip", false).unwrap(),
            10 * 1024 * 1024
        );
        assert_eq!(
            parse_size_with_suffix("500k", "--count", false).unwrap(),
            500 * 1024
        );
        assert_eq!(
            parse_size_with_suffix("1.5GiB", "--input-skip-bits", true).unwrap(),
            (1.5 * 1024.0 * 1024.0 * 1024.0 * 8.0) as u64
        );

        // Multiplication expressions
        assert_eq!(
            parse_size_with_suffix("1000000*24", "--input-skip-bits", true).unwrap(),
            24_000_000
        );
        assert_eq!(
            parse_size_with_suffix("1000*1000*24", "--input-skip-bits", true).unwrap(),
            24_000_000
        );
        assert_eq!(
            parse_size_with_suffix("1M*24", "--input-skip-bits", true).unwrap(),
            1024 * 1024 * 24
        );
        assert_eq!(
            parse_size_with_suffix("1920x1080*24", "--input-skip-bits", true).unwrap(),
            1920 * 1080 * 24
        );
        assert_eq!(
            parse_size_with_suffix("100 * 8B", "--input-skip-bits", true).unwrap(),
            100 * 64
        );
        assert_eq!(
            parse_size_with_suffix("3*8", "--input-unit", false).unwrap(),
            24
        );

        // Integration in CLI validation
        let cli = Cli::parse_from([
            "bdd",
            "--input-zeros",
            "--count=1M",
            "--input-skip-bits=1000000*24",
            "--skip=10K",
            "--input-gap=4*8",
            "--input-repeat=10*2",
        ]);
        let conf = validate_and_process(cli).unwrap();
        assert_eq!(conf.count, Some(1024 * 1024));
        assert_eq!(conf.input_skip_bits, 24_000_000);
        assert_eq!(conf.skip, 10 * 1024);
        assert_eq!(conf.input_gap, 32);
        assert_eq!(conf.input_repeat, 20);
    }
}
