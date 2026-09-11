use crate::error::BddError;
use clap::Parser;

/// Command line arguments for the bdd bitstream utility.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "bdd",
    version = "0.3.0",
    about = "Unix command line program to handle bit streams"
)]
pub struct Cli {
    // File options
    #[arg(long, default_value = "-")]
    pub input_file: String,

    #[arg(long, default_value = "-")]
    pub output_file: String,

    // Input unit and raw unit container selection
    #[arg(long)]
    pub input_unit: Option<usize>,

    /// Size of repeating raw unit / container in bits
    #[arg(long)]
    pub input_raw_unit: Option<usize>,

    #[arg(long, allow_hyphen_values = true)]
    pub input_skip_bits: Option<isize>,

    #[arg(long, allow_hyphen_values = true)]
    pub input_skip_units: Option<isize>,

    /// Bit gap between raw units (alias: --input-postgap)
    #[arg(long, visible_alias = "input-postgap", allow_hyphen_values = true)]
    pub input_gap: Option<isize>,

    /// Bit offset of unit within raw unit (alias: --input-pregap)
    #[arg(long, visible_alias = "input-pregap", allow_hyphen_values = true)]
    pub input_offset: Option<isize>,

    #[arg(long, default_value_t = false)]
    pub input_assert_aligned: bool,

    /// Disable seeking on all inputs (force streaming sequential read)
    #[arg(long, default_value_t = false, visible_alias = "do-not-seek")]
    pub no_seek: bool,

    /// Disable seeking on primary input (force streaming sequential read)
    #[arg(long, default_value_t = false, visible_alias = "no-input-seek")]
    pub input_no_seek: bool,

    /// Enable seeking on primary input (default: true if seekable)
    #[arg(long, default_value_t = false, visible_alias = "input-seek")]
    pub input_use_seek: bool,

    #[arg(long, default_value_t = false)]
    pub input_little_endian: bool,

    #[arg(long, default_value_t = false)]
    pub input_reverse_bytes: bool,

    #[arg(long, default_value_t = false)]
    pub input_reverse_unit: bool,

    // Special input bit streams
    #[arg(long, default_value_t = false)]
    pub input_zeros: bool,

    #[arg(long, default_value_t = false)]
    pub input_ones: bool,

    #[arg(long, default_value_t = false)]
    pub input_random: bool,

    #[arg(long, default_value_t = false)]
    pub input_counter: bool,

    #[arg(long, default_value_t = false)]
    pub input_integers: bool,

    // Tuples
    #[arg(long)]
    pub input_pattern: Option<String>,

    #[arg(long, default_value_t = false)]
    pub input_tuples: bool,

    #[arg(long, allow_hyphen_values = true)]
    pub skip: Option<isize>,

    #[arg(long, allow_hyphen_values = true)]
    pub count: Option<isize>,

    // Manipulate tuples
    #[arg(long, default_missing_value = "", num_args = 0..=1)]
    pub rearrange: Option<String>,

    #[arg(long)]
    pub cut_maxint: Option<String>,

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

    // Pack tuples
    #[arg(long)]
    pub output_pattern: Option<String>,

    #[arg(long, visible_alias = "output-tuple", default_value_t = false)]
    pub output_tuples: bool,

    // Output unit
    #[arg(long)]
    pub output_unit: Option<usize>,

    #[arg(long, default_value_t = false)]
    pub output_little_endian: bool,

    #[arg(long, default_value_t = false)]
    pub output_reverse_bytes: bool,

    #[arg(long, default_value_t = false)]
    pub output_reverse_unit: bool,

    // Special output formats
    #[arg(long, visible_alias = "output-integer", default_value_t = false)]
    pub output_integers: bool,

    #[arg(long, default_value_t = false)]
    pub output_hex: bool,

    #[arg(long, visible_alias = "output-bit", default_value_t = false)]
    pub output_bits: bool,

    #[arg(long, default_value_t = false)]
    pub output_json: bool,

    #[arg(long, default_value_t = false)]
    pub output_csv: bool,

    #[arg(long)]
    pub csv_header: Option<String>,

    #[arg(long, default_value_t = false)]
    pub output_visual: bool,

    // Demux and looping options
    #[arg(long)]
    pub demux: Vec<String>,

    #[arg(long)]
    pub demux_files: Option<String>,

    #[arg(long, default_value_t = 1)]
    pub input_repeat: usize,

    // Merge options
    #[arg(long)]
    pub merge_file: Option<String>,

    #[arg(long)]
    pub merge_unit: Option<usize>,

    #[arg(long, allow_hyphen_values = true)]
    pub merge_skip_bits: Option<isize>,

    #[arg(long, allow_hyphen_values = true)]
    pub merge_skip_units: Option<isize>,

    #[arg(long, allow_hyphen_values = true)]
    pub merge_copy_first: Option<isize>,

    /// Size of repeating raw unit / container in bits for merge stream
    #[arg(long)]
    pub merge_raw_unit: Option<usize>,

    #[arg(long, allow_hyphen_values = true)]
    pub merge_gap: Option<isize>,

    /// Bit offset of unit within merge raw unit (alias: --merge-pregap)
    #[arg(long, visible_alias = "merge-pregap", allow_hyphen_values = true)]
    pub merge_offset: Option<isize>,

    #[arg(long, default_value_t = false)]
    pub merge_assert_aligned: bool,

    /// Disable seeking on merge input (force streaming sequential read)
    #[arg(long, default_value_t = false, visible_alias = "no-merge-seek")]
    pub merge_no_seek: bool,

    /// Enable seeking on merge input (default: true if seekable)
    #[arg(long, default_value_t = false, visible_alias = "merge-seek")]
    pub merge_use_seek: bool,

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
    pub input_skip_bits: usize,
    pub input_skip_units: usize,
    pub input_gap: usize,
    pub input_assert_aligned: bool,
    pub input_use_seek: bool,
    pub input_reverse_bytes: bool,
    pub input_reverse_unit: bool,
    pub input_zeros: bool,
    pub input_ones: bool,
    pub input_random: bool,
    pub input_counter: bool,
    pub input_integers: bool,
    pub input_pattern: Option<String>,
    pub input_tuples: bool,
    pub skip: usize,
    pub count: Option<usize>,
    pub rearrange: Option<String>,
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
    pub output_pattern: Option<String>,
    pub output_tuples: bool,
    pub output_unit: Option<usize>,
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
    pub merge_file: Option<String>,
    pub merge_unit: Option<usize>,
    pub merge_skip_bits: usize,
    pub merge_skip_units: usize,
    pub merge_copy_first: usize,
    pub merge_gap: usize,
    pub merge_assert_aligned: bool,
    pub merge_use_seek: bool,
    pub merge_reverse_bytes: bool,
    pub merge_reverse_unit: bool,
    pub raw_args: Vec<String>,
}

fn check_exclusive(msg: &str, flags: &[bool]) -> Result<(), BddError> {
    let count = flags.iter().filter(|&&b| b).count();
    if count > 1 {
        return Err(BddError::CliError(msg.to_string()));
    }
    Ok(())
}

fn check_number_argument(
    val: Option<isize>,
    option_name: &str,
    default: Option<usize>,
) -> Option<usize> {
    match val {
        None => default,
        Some(i) => {
            if i >= 0 {
                Some(i as usize)
            } else {
                eprintln!("{} value must be a positive integer", option_name);
                match default {
                    Some(d) => {
                        eprintln!("Assumed {}={}", option_name, d);
                        Some(d)
                    }
                    None => {
                        eprintln!("Assumed {}=None", option_name);
                        None
                    }
                }
            }
        }
    }
}

/// Validate CLI flags against legacy exclusivity rules and calculate effective options.
pub fn validate_and_process(mut cli: Cli) -> Result<ValidatedConfig, BddError> {
    check_exclusive(
        "Only one of the following is allowed: --input-zeros, --input-ones, --input-random, --input-counter,--input-integers, --input-tuples",
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

    let skip = check_number_argument(cli.skip, "--skip", Some(0)).unwrap_or(0);
    let count = check_number_argument(cli.count, "--count", None);

    if has_special_stream && count.is_none() {
        return Err(BddError::CliError(
            "--input-zeros, --input-ones, --input-random and --input-counter require --count"
                .to_string(),
        ));
    }

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

    if cli.input_pattern.is_some() && (cli.input_unit.is_some() || cli.input_raw_unit.is_some()) {
        return Err(BddError::CliError(
            "--in/output-pattern overwrites --in/output-unit and --input-raw-unit, use either one"
                .to_string(),
        ));
    }
    if cli.output_pattern.is_some() && cli.output_unit.is_some() {
        return Err(BddError::CliError(
            "--in/output-pattern overwrites --in/output-unit, use either one".to_string(),
        ));
    }

    let merge_specified = cli.merge_file.is_some();
    if !merge_specified {
        let has_merge_opt = cli.merge_unit.is_some()
            || cli.merge_raw_unit.is_some()
            || cli.merge_skip_bits.is_some()
            || cli.merge_skip_units.is_some()
            || cli.merge_copy_first.is_some()
            || cli.merge_gap.is_some()
            || cli.merge_offset.is_some()
            || cli.merge_assert_aligned
            || cli.merge_use_seek
            || cli.merge_no_seek
            || cli.merge_little_endian
            || cli.merge_reverse_bytes
            || cli.merge_reverse_unit;
        if has_merge_opt {
            return Err(BddError::CliError(
                "Merge options can be used only if --merge-file option is set".to_string(),
            ));
        }
    }

    let raw_unit = cli.input_raw_unit;
    let input_offset =
        check_number_argument(cli.input_offset, "--input-offset", Some(0)).unwrap_or(0);
    let input_gap_raw = check_number_argument(cli.input_gap, "--input-gap", Some(0)).unwrap_or(0);
    let input_skip_bits_raw =
        check_number_argument(cli.input_skip_bits, "--input-skip-bits", Some(0)).unwrap_or(0);
    let input_skip_units_raw =
        check_number_argument(cli.input_skip_units, "--input-skip-units", Some(0)).unwrap_or(0);

    let (input_skip_bits, input_skip_units, input_gap, resolved_input_unit) = if let Some(r) =
        raw_unit
    {
        if r == 0 {
            return Err(BddError::CliError(
                "--input-raw-unit must be greater than 0".to_string(),
            ));
        }
        let u = if let Some(unit) = cli.input_unit {
            if unit == 0 {
                return Err(BddError::CliError(
                    "--input-unit must be greater than 0".to_string(),
                ));
            }
            if input_offset + unit > r {
                return Err(BddError::CliError(format!(
                    "--input-offset ({}) + --input-unit ({}) exceeds --input-raw-unit ({})",
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
            r - input_offset
        };
        let skip_stride = r + input_gap_raw;
        let skip_bits = input_skip_bits_raw + (input_skip_units_raw * skip_stride) + input_offset;
        let gap = r - u + input_gap_raw;
        (skip_bits, 0, gap, Some(u))
    } else {
        let skip_bits = input_skip_bits_raw + input_offset;
        let gap = input_gap_raw + input_offset;
        (skip_bits, input_skip_units_raw, gap, cli.input_unit)
    };

    let merge_raw_unit = cli.merge_raw_unit;
    let merge_offset =
        check_number_argument(cli.merge_offset, "--merge-offset", Some(0)).unwrap_or(0);
    let merge_gap_raw = check_number_argument(cli.merge_gap, "--merge-gap", Some(0)).unwrap_or(0);
    let merge_skip_bits_raw =
        check_number_argument(cli.merge_skip_bits, "--merge-skip-bits", Some(0)).unwrap_or(0);
    let merge_skip_units_raw =
        check_number_argument(cli.merge_skip_units, "--merge-skip-units", Some(0)).unwrap_or(0);
    let merge_copy_first =
        check_number_argument(cli.merge_copy_first, "--merge-copy-first", Some(0)).unwrap_or(0);

    let (merge_skip_bits, merge_skip_units, merge_gap, resolved_merge_unit) = if let Some(r) =
        merge_raw_unit
    {
        if r == 0 {
            return Err(BddError::CliError(
                "--merge-raw-unit must be greater than 0".to_string(),
            ));
        }
        let u = if let Some(unit) = cli.merge_unit {
            if unit == 0 {
                return Err(BddError::CliError(
                    "--merge-unit must be greater than 0".to_string(),
                ));
            }
            if merge_offset + unit > r {
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
            r - merge_offset
        };
        let skip_stride = r + merge_gap_raw;
        let skip_bits = merge_skip_bits_raw + (merge_skip_units_raw * skip_stride) + merge_offset;
        let gap = r - u + merge_gap_raw;
        (skip_bits, 0, gap, Some(u))
    } else {
        let skip_bits = merge_skip_bits_raw + merge_offset;
        let gap = merge_gap_raw + merge_offset;
        (skip_bits, merge_skip_units_raw, gap, cli.merge_unit)
    };

    if cli.input_little_endian {
        if cli.input_reverse_bytes || cli.input_reverse_unit {
            eprintln!(
                "--input-little-endian overwrites --input-reverse-bytes and --input-reverse-unit"
            );
        }
        cli.input_reverse_bytes = true;
        cli.input_reverse_unit = true;
    }

    if cli.output_little_endian {
        if cli.output_reverse_bytes || cli.output_reverse_unit {
            eprintln!("--output-little-endian overwrites --output-reverse-bytes and --output-reverse-unit");
        }
        cli.output_reverse_bytes = true;
        cli.output_reverse_unit = true;
    }

    if cli.merge_little_endian {
        if cli.merge_reverse_bytes || cli.merge_reverse_unit {
            eprintln!(
                "--merge-little-endian overwrites --merge-reverse-bytes and --merge-reverse-unit"
            );
        }
        cli.merge_reverse_bytes = true;
        cli.merge_reverse_unit = true;
    }

    if cli.input_file == "-" && cli.input_use_seek {
        eprintln!("Standard input does not allow seek.--input-use-seek disabled");
        cli.input_use_seek = false;
    }

    // Auto-seek by default on regular files / seekable descriptors, unless explicitly disabled
    let input_use_seek = !cli.input_no_seek && !cli.no_seek;
    let merge_use_seek = !cli.merge_no_seek && !cli.no_seek;

    Ok(ValidatedConfig {
        input_file: cli.input_file,
        output_file: cli.output_file,
        input_unit: resolved_input_unit,
        input_skip_bits,
        input_skip_units,
        input_gap,
        input_assert_aligned: cli.input_assert_aligned,
        input_use_seek,
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
        cut_maxint: cli.cut_maxint,
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
        output_pattern: cli.output_pattern,
        output_tuples: cli.output_tuples,
        output_unit: cli.output_unit,
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
        input_repeat: cli.input_repeat,
        merge_file: cli.merge_file,
        merge_unit: resolved_merge_unit,
        merge_skip_bits,
        merge_skip_units,
        merge_copy_first,
        merge_gap,
        merge_assert_aligned: cli.merge_assert_aligned,
        merge_use_seek,
        merge_reverse_bytes: cli.merge_reverse_bytes,
        merge_reverse_unit: cli.merge_reverse_unit,
        raw_args: Vec::new(),
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

        // Global --no-seek disables both
        let cli_no_seek = Cli::parse_from(["bdd", "--no-seek", "--input-zeros", "--count=1"]);
        let conf_no_seek = validate_and_process(cli_no_seek).unwrap();
        assert!(!conf_no_seek.input_use_seek);
        assert!(!conf_no_seek.merge_use_seek);

        // Alias --do-not-seek
        let cli_do_not_seek =
            Cli::parse_from(["bdd", "--do-not-seek", "--input-zeros", "--count=1"]);
        let conf_do_not_seek = validate_and_process(cli_do_not_seek).unwrap();
        assert!(!conf_do_not_seek.input_use_seek);

        // --input-no-seek disables only input
        let cli_input_no_seek =
            Cli::parse_from(["bdd", "--input-no-seek", "--input-zeros", "--count=1"]);
        let conf_input_no_seek = validate_and_process(cli_input_no_seek).unwrap();
        assert!(!conf_input_no_seek.input_use_seek);
        assert!(conf_input_no_seek.merge_use_seek);

        // --merge-no-seek with merge file
        let cli_merge_no_seek = Cli::parse_from([
            "bdd",
            "--input-zeros",
            "--count=1",
            "--merge-file",
            "/dev/null",
            "--merge-no-seek",
        ]);
        let conf_merge_no_seek = validate_and_process(cli_merge_no_seek).unwrap();
        assert!(conf_merge_no_seek.input_use_seek);
        assert!(!conf_merge_no_seek.merge_use_seek);

        // --merge-no-seek without merge-file fails validation
        let cli_merge_err =
            Cli::parse_from(["bdd", "--input-zeros", "--count=1", "--merge-no-seek"]);
        assert!(validate_and_process(cli_merge_err).is_err());
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
}
