//! # bdd
//!
//! A command line tool and library to interpret, manipulate, merge, and create
//! bit streams of arbitrary bit size with advanced bit field interpretation.

pub mod cli;
pub mod counter;
pub mod engine;
pub mod error;
pub mod field;
pub mod manipulator;
pub mod pattern;
pub mod sink;
pub mod stream;

pub use engine::run_pipeline;
pub use error::BddError;
pub use field::Field;
