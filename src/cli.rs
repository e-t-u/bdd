use clap::Parser;

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

    // Input unit selection
    #[arg(long)]
    pub input_unit: Option<usize>,

    #[arg(long, allow_hyphen_values = true)]
    pub input_skip_bits: Option<isize>,

    #[arg(long, allow_hyphen_values = true)]
    pub input_skip_units: Option<isize>,

    #[arg(long, allow_hyphen_values = true)]
    pub input_gap: Option<isize>,

    #[arg(long, allow_hyphen_values = true)]
    pub input_pregap: Option<isize>,

    #[arg(long, default_value_t = false)]
    pub input_assert_aligned: bool,

    #[arg(long, default_value_t = false)]
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
    pub xor: Option<String>,

    #[arg(long)]
    pub abs: Option<String>,

    #[arg(long)]
    pub sign: Option<String>,

    // Pack tuples
    #[arg(long)]
    pub output_pattern: Option<String>,

    #[arg(long, default_value_t = false)]
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

    // Special output for units
    #[arg(long, visible_alias = "output-integer", default_value_t = false)]
    pub output_integers: bool,

    #[arg(long, default_value_t = false)]
    pub output_hex: bool,

    #[arg(long, default_value_t = false)]
    pub output_bits: bool,

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

    #[arg(long, allow_hyphen_values = true)]
    pub merge_gap: Option<isize>,

    #[arg(long, allow_hyphen_values = true)]
    pub merge_pregap: Option<isize>,

    #[arg(long, default_value_t = false)]
    pub merge_assert_aligned: bool,

    #[arg(long, default_value_t = false)]
    pub merge_use_seek: bool,

    #[arg(long, default_value_t = false)]
    pub merge_little_endian: bool,

    #[arg(long, default_value_t = false)]
    pub merge_reverse_bytes: bool,

    #[arg(long, default_value_t = false)]
    pub merge_reverse_unit: bool,
}

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
    pub xor: Option<String>,
    pub abs: Option<String>,
    pub sign: Option<String>,
    pub output_pattern: Option<String>,
    pub output_tuples: bool,
    pub output_unit: Option<usize>,
    pub output_reverse_bytes: bool,
    pub output_reverse_unit: bool,
    pub output_integers: bool,
    pub output_hex: bool,
    pub output_bits: bool,
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
}

fn error_exit(msg: &str) -> ! {
    eprintln!("Usage: bdd [options]\n\nbdd: error: {}", msg);
    std::process::exit(2);
}

fn check_exclusive(msg: &str, flags: &[bool]) {
    let count = flags.iter().filter(|&&b| b).count();
    if count > 1 {
        error_exit(msg);
    }
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

pub fn validate_and_process(mut cli: Cli) -> ValidatedConfig {
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
    );

    let has_special_stream =
        cli.input_zeros || cli.input_ones || cli.input_random || cli.input_counter;

    if cli.input_file != "-" && has_special_stream {
        error_exit(
            "--input-file can not be used with --input-zeros, --input-ones, --input-random or --input-counter",
        );
    }

    let skip = check_number_argument(cli.skip, "--skip", Some(0)).unwrap_or(0);
    let count = check_number_argument(cli.count, "--count", None);

    if has_special_stream && count.is_none() {
        error_exit(
            "--input-zeros, --input-ones, --input-random and --input-counter require --count",
        );
    }

    check_exclusive(
        "Only one of the following: --output-tuples, --output-integers, --output-hex, --output-bits",
        &[
            cli.output_tuples,
            cli.output_integers,
            cli.output_hex,
            cli.output_bits,
        ],
    );

    if cli.input_pattern.is_some() && cli.input_unit.is_some() {
        error_exit("--in/output-pattern overwrites --in/output-unit, use either one");
    }
    if cli.output_pattern.is_some() && cli.output_unit.is_some() {
        error_exit("--in/output-pattern overwrites --in/output-unit, use either one");
    }

    let merge_specified = cli.merge_file.is_some();
    if !merge_specified {
        let has_merge_opt = cli.merge_unit.is_some()
            || cli.merge_skip_bits.is_some()
            || cli.merge_skip_units.is_some()
            || cli.merge_copy_first.is_some()
            || cli.merge_gap.is_some()
            || cli.merge_pregap.is_some()
            || cli.merge_assert_aligned
            || cli.merge_use_seek
            || cli.merge_little_endian
            || cli.merge_reverse_bytes
            || cli.merge_reverse_unit;
        if has_merge_opt {
            error_exit("Merge options can be used only if --merge-file option is set");
        }
    }

    let mut input_skip_bits =
        check_number_argument(cli.input_skip_bits, "--input-skip-bits", Some(0)).unwrap_or(0);
    let input_skip_units =
        check_number_argument(cli.input_skip_units, "--input-skip-units", Some(0)).unwrap_or(0);
    let mut input_gap =
        check_number_argument(cli.input_gap, "--input-gap", Some(0)).unwrap_or(0);
    let input_pregap =
        check_number_argument(cli.input_pregap, "--input-pregap", Some(0)).unwrap_or(0);

    input_skip_bits += input_pregap;
    input_gap += input_pregap;

    let mut merge_skip_bits =
        check_number_argument(cli.merge_skip_bits, "--merge-skip-bits", Some(0)).unwrap_or(0);
    let merge_skip_units =
        check_number_argument(cli.merge_skip_units, "--merge-skip-units", Some(0)).unwrap_or(0);
    let merge_copy_first =
        check_number_argument(cli.merge_copy_first, "--merge-copy-first", Some(0)).unwrap_or(0);
    let mut merge_gap =
        check_number_argument(cli.merge_gap, "--merge-gap", Some(0)).unwrap_or(0);
    let merge_pregap =
        check_number_argument(cli.merge_pregap, "--merge-pregap", Some(0)).unwrap_or(0);

    merge_skip_bits += merge_pregap;
    merge_gap += merge_pregap;

    if cli.input_little_endian {
        if cli.input_reverse_bytes || cli.input_reverse_unit {
            eprintln!("--input-little-endian overwrites --input-reverse-bytes and --input-reverse-unit");
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
            eprintln!("--merge-little-endian overwrites --merge-reverse-bytes and --merge-reverse-unit");
        }
        cli.merge_reverse_bytes = true;
        cli.merge_reverse_unit = true;
    }

    if cli.input_file == "-" && cli.input_use_seek {
        eprintln!("Standard input does not allow seek.--input-use-seek disabled");
        cli.input_use_seek = false;
    }

    ValidatedConfig {
        input_file: cli.input_file,
        output_file: cli.output_file,
        input_unit: cli.input_unit,
        input_skip_bits,
        input_skip_units,
        input_gap,
        input_assert_aligned: cli.input_assert_aligned,
        input_use_seek: cli.input_use_seek,
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
        xor: cli.xor,
        abs: cli.abs,
        sign: cli.sign,
        output_pattern: cli.output_pattern,
        output_tuples: cli.output_tuples,
        output_unit: cli.output_unit,
        output_reverse_bytes: cli.output_reverse_bytes,
        output_reverse_unit: cli.output_reverse_unit,
        output_integers: cli.output_integers,
        output_hex: cli.output_hex,
        output_bits: cli.output_bits,
        merge_file: cli.merge_file,
        merge_unit: cli.merge_unit,
        merge_skip_bits,
        merge_skip_units,
        merge_copy_first,
        merge_gap,
        merge_assert_aligned: cli.merge_assert_aligned,
        merge_use_seek: cli.merge_use_seek,
        merge_reverse_bytes: cli.merge_reverse_bytes,
        merge_reverse_unit: cli.merge_reverse_unit,
    }
}
