//! # bdd
//!
//! A command line tool and library to interpret, manipulate, merge, and create
//! bit streams of arbitrary bit size with advanced bit field interpretation.

pub mod bits;
pub mod cli;
pub mod counter;
pub mod diag;
pub mod engine;
pub mod error;
pub mod explain;
pub mod ffi;
pub mod field;
#[cfg(feature = "small-floats")]
pub mod float_types;
pub mod manipulator;
pub mod mcp;
pub mod pattern;
pub mod preset;
pub mod probe;
#[cfg(feature = "server")]
pub mod server;
pub mod sink;
pub mod stream;
pub mod stream_pattern;

pub use bits::{
    copy_bits, read_bits_biguint, read_bits_u64, write_bits_biguint, write_bits_u64,
    BitStreamReader, BitStreamWriter,
};
pub use engine::run_pipeline;
pub use error::BddError;
pub use field::Field;
pub use stream::{BddReader, ReadSeek, StreamSeek};
