//! # bdd
//!
//! A command line tool and library to interpret, manipulate, merge, and create
//! bit streams of arbitrary bit size with advanced bit field interpretation.

pub mod cli;
pub mod counter;
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

pub use engine::run_pipeline;
pub use error::BddError;
pub use field::Field;
pub use stream::{BddReader, ReadSeek, StreamSeek};
