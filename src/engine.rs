use crate::cli::ValidatedConfig;
use crate::counter::Counter;
use crate::error::BddError;
use crate::field::Field;
use crate::manipulator::*;
use crate::pattern::{TuplePacker, TupleUnpacker};
use crate::sink::{
    BitOutputStream, FileOutputStream, HexOutputStream, IntegerOutputStream, TupleDirectOutput,
    UnitSink,
};
use crate::stream::{
    CounterStream, FileInputStream, IntegerInputStream, OneStream, RandomStream, StreamConfig,
    TupleDirectInput, UnitStream, ZeroStream,
};
use num_bigint::BigUint;
use num_traits::Zero;
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

/// Runs the complete bdd pipeline based on validated configuration.
pub fn run_pipeline(config: ValidatedConfig) -> Result<(), BddError> {
    let unpacker = if let Some(ref p) = config.input_pattern {
        Some(TupleUnpacker::new(p)?)
    } else {
        None
    };

    let in_unit_size = if let Some(u) = config.input_unit {
        u
    } else if let Some(ref u) = unpacker {
        u.total_bits
    } else {
        8
    };

    let packer = if let Some(ref p) = config.output_pattern {
        Some(TuplePacker::new(p)?)
    } else {
        None
    };

    let out_unit_size = if let Some(u) = config.output_unit {
        u
    } else if let Some(ref p) = packer {
        p.total_bits
    } else {
        8
    };

    let mut merge_stream: Option<FileInputStream<Box<dyn Read>>> =
        if let Some(ref mfile) = config.merge_file {
            let reader: Box<dyn Read> = if mfile == "-" {
                Box::new(io::stdin())
            } else {
                match File::open(mfile) {
                    Ok(f) => Box::new(BufReader::new(f)),
                    Err(_) => {
                        return Err(BddError::CannotOpenMergeFile(mfile.clone()));
                    }
                }
            };
            let m_unit = config.merge_unit.unwrap_or(8);
            let stream_conf = StreamConfig {
                skip_bits: config.merge_skip_bits,
                skip_units: config.merge_skip_units,
                gap: config.merge_gap,
                assert_aligned: config.merge_assert_aligned,
                reverse_bytes: config.merge_reverse_bytes,
                reverse_unit: config.merge_reverse_unit,
                unit_size: m_unit,
            };
            let mut ms = FileInputStream::new(reader, stream_conf, Counter::new(0, None));
            ms.do_skip();
            Some(ms)
        } else {
            None
        };

    let mut manipulators: Vec<Box<dyn TupleManipulator>> = Vec::new();
    if let Some(ref arg) = config.rearrange {
        manipulators.push(Box::new(RearrangeManipulator::new(arg)?));
    }
    if let Some(ref arg) = config.cut_maxint {
        manipulators.push(Box::new(CutMaxintManipulator::new(arg)?));
    }
    if let Some(ref arg) = config.remove_right {
        manipulators.push(Box::new(RemoveRightManipulator::new(arg)?));
    }
    if let Some(ref arg) = config.xor {
        manipulators.push(Box::new(XorManipulator::new(arg)?));
    }
    if let Some(ref arg) = config.abs {
        manipulators.push(Box::new(AbsManipulator::new(arg)?));
    }
    if let Some(ref arg) = config.sign {
        manipulators.push(Box::new(SignManipulator::new(arg)?));
    }

    let out_writer: Box<dyn Write> = if config.output_file == "-" {
        Box::new(io::stdout())
    } else {
        match File::create(&config.output_file) {
            Ok(f) => Box::new(BufWriter::new(f)),
            Err(_) => return Err(BddError::CannotOpenOutputFile),
        }
    };

    let mut tuple_direct_out: Option<TupleDirectOutput<Box<dyn Write>>> = None;
    let mut unit_sink: Option<Box<dyn UnitSink>> = None;

    if config.output_tuples {
        tuple_direct_out = Some(TupleDirectOutput::new(out_writer));
    } else if config.output_integers {
        unit_sink = Some(Box::new(IntegerOutputStream::new(out_writer)));
    } else if config.output_hex {
        unit_sink = Some(Box::new(HexOutputStream::new(out_writer)));
    } else if config.output_bits {
        unit_sink = Some(Box::new(BitOutputStream::new(out_writer)));
    } else {
        unit_sink = Some(Box::new(FileOutputStream::new(
            out_writer,
            config.output_reverse_bytes,
            config.output_reverse_unit,
        )));
    }

    if let Some(ref mut ms) = merge_stream {
        if config.merge_copy_first > 0 {
            let bits = ms.read_bits(config.merge_copy_first);
            if let Some(ref mut sink) = unit_sink {
                sink.write_bits(bits, config.merge_copy_first)?;
            }
        }
    }

    let counter = Counter::new(config.skip, config.count);

    if config.input_tuples {
        let in_reader: Box<dyn BufRead> = if config.input_file == "-" {
            Box::new(BufReader::new(io::stdin()))
        } else {
            match File::open(&config.input_file) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(_) => return Err(BddError::CannotOpenInputFile(config.input_file.clone())),
            }
        };
        let mut tuple_in = TupleDirectInput::new(in_reader, counter);

        while let Some(mut tuple) = tuple_in.next_tuple()? {
            for m in &manipulators {
                tuple = m.manipulate(tuple);
            }
            if let Some(ref mut tout) = tuple_direct_out {
                tout.write_tuple(&tuple)?;
            } else {
                let unit = if let Some(ref p) = packer {
                    p.pack(tuple)?
                } else if !tuple.is_empty() {
                    tuple[0].as_biguint()
                } else {
                    eprintln!("No fields to output, assumed 0");
                    BigUint::zero()
                };
                if let Some(ref mut sink) = unit_sink {
                    sink.write_bits(unit, out_unit_size)?;
                }
            }
            if let Some(ref mut ms) = merge_stream {
                let m_unit = ms.config.unit_size;
                match ms.next_unit()? {
                    Some(u) => {
                        if let Some(ref mut sink) = unit_sink {
                            sink.write_bits(u, m_unit)?;
                        }
                    }
                    None => {
                        eprintln!("Premature end of merge file");
                        break;
                    }
                }
            }
        }
    } else {
        let mut unit_stream: Box<dyn UnitStream> = if config.input_zeros {
            Box::new(ZeroStream::new(counter))
        } else if config.input_ones {
            Box::new(OneStream::new(counter, in_unit_size))
        } else if config.input_random {
            Box::new(RandomStream::new(counter, in_unit_size))
        } else if config.input_counter {
            Box::new(CounterStream::new(counter, in_unit_size))
        } else if config.input_integers {
            let in_reader: Box<dyn BufRead> = if config.input_file == "-" {
                Box::new(BufReader::new(io::stdin()))
            } else {
                match File::open(&config.input_file) {
                    Ok(f) => Box::new(BufReader::new(f)),
                    Err(_) => return Err(BddError::CannotOpenInputFile(config.input_file.clone())),
                }
            };
            Box::new(IntegerInputStream::new(in_reader, counter))
        } else {
            let in_reader: Box<dyn Read> = if config.input_file == "-" {
                Box::new(io::stdin())
            } else {
                match File::open(&config.input_file) {
                    Ok(f) => Box::new(BufReader::new(f)),
                    Err(_) => return Err(BddError::CannotOpenInputFile(config.input_file.clone())),
                }
            };
            let stream_conf = StreamConfig {
                skip_bits: config.input_skip_bits,
                skip_units: config.input_skip_units,
                gap: config.input_gap,
                assert_aligned: config.input_assert_aligned,
                reverse_bytes: config.input_reverse_bytes,
                reverse_unit: config.input_reverse_unit,
                unit_size: in_unit_size,
            };
            let mut fs = FileInputStream::new(in_reader, stream_conf, counter);
            fs.do_skip();
            Box::new(fs)
        };

        while let Some(unit) = unit_stream.next_unit()? {
            let mut tuple = if let Some(ref u) = unpacker {
                u.unpack(unit)
            } else {
                vec![Field::UInt(unit)]
            };

            for m in &manipulators {
                tuple = m.manipulate(tuple);
            }

            if let Some(ref mut tout) = tuple_direct_out {
                tout.write_tuple(&tuple)?;
            } else {
                let out_unit = if let Some(ref p) = packer {
                    p.pack(tuple)?
                } else if !tuple.is_empty() {
                    tuple[0].as_biguint()
                } else {
                    eprintln!("No fields to output, assumed 0");
                    BigUint::zero()
                };
                if let Some(ref mut sink) = unit_sink {
                    sink.write_bits(out_unit, out_unit_size)?;
                }
            }

            if let Some(ref mut ms) = merge_stream {
                let m_unit = ms.config.unit_size;
                match ms.next_unit()? {
                    Some(u) => {
                        if let Some(ref mut sink) = unit_sink {
                            sink.write_bits(u, m_unit)?;
                        }
                    }
                    None => {
                        eprintln!("Premature end of merge file");
                        break;
                    }
                }
            }
        }
    }

    if let Some(ref mut tout) = tuple_direct_out {
        tout.flush_stream()?;
    }
    if let Some(ref mut sink) = unit_sink {
        sink.flush_stream()?;
    }

    Ok(())
}
