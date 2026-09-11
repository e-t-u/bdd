use std::fmt;

/// Central error type for all bdd operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BddError {
    CliError(String),
    MissingInputPattern,
    MissingOutputPattern,
    IllegalInputPatternChar(char),
    IllegalOutputPatternChar(char),
    InputBitLengthRequired(char),
    OutputBitLengthRequired(char),
    InvalidInputBitLength(char, usize),
    InvalidOutputBitLength(char, usize),
    NonAlignedEof,
    InputHasLessFields,
    RearrangeNonNumber,
    ManipulatorArgumentError(String),
    CannotOpenInputFile(String),
    CannotOpenOutputFile,
    CannotOpenMergeFile(String),
    SeekFailed,
    IoError(String),
}

impl BddError {
    /// Return the UNIX process exit code corresponding to this error.
    pub fn exit_code(&self) -> i32 {
        match self {
            BddError::CliError(_) => 2,
            _ => 1,
        }
    }
}

impl fmt::Display for BddError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BddError::CliError(msg) => write!(f, "Usage: bdd [options]\n\nbdd: error: {}", msg),
            BddError::MissingInputPattern => write!(f, "Missing input pattern"),
            BddError::MissingOutputPattern => write!(f, "Missing output pattern"),
            BddError::IllegalInputPatternChar(c) => {
                write!(f, "Illegal character {} in input-pattern", c)
            }
            BddError::IllegalOutputPatternChar(c) => {
                write!(f, "Illegal character {} in output-pattern", c)
            }
            BddError::InputBitLengthRequired(c) => write!(
                f,
                "Character {} in input pattern requires number of bits",
                c
            ),
            BddError::OutputBitLengthRequired(c) => write!(
                f,
                "Character {} in output pattern requires number of bits",
                c
            ),
            BddError::InvalidInputBitLength(c, expected) => {
                write!(
                    f,
                    "Number of bits for input pattern {} must be {}",
                    c, expected
                )
            }
            BddError::InvalidOutputBitLength(c, expected) => {
                write!(
                    f,
                    "Number of bits for output pattern {} must be {}",
                    c, expected
                )
            }
            BddError::NonAlignedEof => write!(f, "Non-aligned end of file"),
            BddError::InputHasLessFields => {
                write!(f, "Input has less fields than in output pattern")
            }
            BddError::RearrangeNonNumber => write!(f, "Fields in --rearrange must be numbers"),
            BddError::ManipulatorArgumentError(msg) => write!(f, "{}", msg),
            BddError::CannotOpenInputFile(path) => write!(f, "Can not open input file {}", path),
            BddError::CannotOpenOutputFile => write!(f, "Can not open output file"),
            BddError::CannotOpenMergeFile(path) => write!(f, "Can not open merge file {}", path),
            BddError::SeekFailed => write!(f, "seek failed"),
            BddError::IoError(msg) => write!(f, "IO error: {}", msg),
        }
    }
}

impl std::error::Error for BddError {}

impl From<std::io::Error> for BddError {
    fn from(err: std::io::Error) -> Self {
        BddError::IoError(err.to_string())
    }
}
