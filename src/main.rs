mod cli;
mod counter;
mod field;
mod manipulator;
mod pattern;
mod sink;
mod stream;

use clap::Parser;
use cli::{validate_and_process, Cli};
use counter::Counter;
use field::Field;
use manipulator::*;
use num_bigint::BigUint;
use num_traits::Zero;
use pattern::{TuplePacker, TupleUnpacker};
use sink::{
    BitOutputStream, FileOutputStream, HexOutputStream, IntegerOutputStream, TupleDirectOutput,
    UnitSink,
};
use stream::{
    CounterStream, FileInputStream, IntegerInputStream, OneStream, RandomStream, TupleDirectInput,
    UnitStream, ZeroStream,
};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let config = validate_and_process(cli);

    // Prepare unpacker if input pattern specified
    let unpacker = config
        .input_pattern
        .as_ref()
        .map(|p| TupleUnpacker::new(p));

    // Determine input unit size
    let in_unit_size = if let Some(u) = config.input_unit {
        u
    } else if let Some(ref u) = unpacker {
        u.total_bits
    } else {
        8
    };

    // Prepare packer if output pattern specified
    let packer = config.output_pattern.as_ref().map(|p| TuplePacker::new(p));

    // Determine output unit size
    let out_unit_size = if let Some(u) = config.output_unit {
        u
    } else if let Some(ref p) = packer {
        p.total_bits
    } else {
        8
    };

    // Open merge file if specified
    let mut merge_stream: Option<FileInputStream<Box<dyn Read>>> = if let Some(ref mfile) =
        config.merge_file
    {
        let reader: Box<dyn Read> = if mfile == "-" {
            Box::new(io::stdin())
        } else {
            match File::open(mfile) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(_) => {
                    eprintln!("Can not open merge file {}", mfile);
                    std::process::exit(1);
                }
            }
        };
        let m_unit = config.merge_unit.unwrap_or(8);
        let mut ms = FileInputStream::new(
            reader,
            config.merge_skip_bits,
            config.merge_skip_units,
            config.merge_gap,
            config.merge_assert_aligned,
            config.merge_use_seek,
            config.merge_reverse_bytes,
            config.merge_reverse_unit,
            Counter::new(0, None),
        );
        ms.unit_size = m_unit;
        ms.do_skip();
        Some(ms)
    } else {
        None
    };

    // Setup manipulators
    let mut manipulators: Vec<Box<dyn TupleManipulator>> = Vec::new();
    if let Some(ref arg) = config.rearrange {
        manipulators.push(Box::new(RearrangeManipulator::new(arg)));
    }
    if let Some(ref arg) = config.cut_maxint {
        manipulators.push(Box::new(CutMaxintManipulator::new(arg)));
    }
    if let Some(ref arg) = config.remove_right {
        manipulators.push(Box::new(RemoveRightManipulator::new(arg)));
    }
    if let Some(ref arg) = config.xor {
        manipulators.push(Box::new(XorManipulator::new(arg)));
    }
    if let Some(ref arg) = config.abs {
        manipulators.push(Box::new(AbsManipulator::new(arg)));
    }
    if let Some(ref arg) = config.sign {
        manipulators.push(Box::new(SignManipulator::new(arg)));
    }

    // Setup output sink
    let out_writer: Box<dyn Write> = if config.output_file == "-" {
        Box::new(io::stdout())
    } else {
        match File::create(&config.output_file) {
            Ok(f) => Box::new(BufWriter::new(f)),
            Err(_) => {
                eprintln!("Can not open output file");
                std::process::exit(1);
            }
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

    // Handle merge_copy_first
    if let Some(ref mut ms) = merge_stream {
        if config.merge_copy_first > 0 {
            let bits = ms.read_bits(config.merge_copy_first);
            if let Some(ref mut sink) = unit_sink {
                sink.write_bits(bits, config.merge_copy_first)?;
            }
        }
    }

    // Setup input and execution
    let counter = Counter::new(config.skip, config.count);

    if config.input_tuples {
        let in_reader: Box<dyn BufRead> = if config.input_file == "-" {
            Box::new(BufReader::new(io::stdin()))
        } else {
            match File::open(&config.input_file) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(_) => {
                    eprintln!("Can not open input file {}", config.input_file);
                    std::process::exit(1);
                }
            }
        };
        let mut tuple_in = TupleDirectInput::new(in_reader, counter);

        while let Some(mut tuple) = tuple_in.next_tuple() {
            for m in &manipulators {
                tuple = m.manipulate(tuple);
            }
            if let Some(ref mut tout) = tuple_direct_out {
                tout.write_tuple(&tuple)?;
            } else {
                let unit = if let Some(ref p) = packer {
                    p.pack(tuple)
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
                let m_unit = ms.unit_size;
                match ms.next_unit() {
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
                    Err(_) => {
                        eprintln!("Can not open input file {}", config.input_file);
                        std::process::exit(1);
                    }
                }
            };
            Box::new(IntegerInputStream::new(in_reader, counter))
        } else {
            let in_reader: Box<dyn Read> = if config.input_file == "-" {
                Box::new(io::stdin())
            } else {
                match File::open(&config.input_file) {
                    Ok(f) => Box::new(BufReader::new(f)),
                    Err(_) => {
                        eprintln!("Can not open input file {}", config.input_file);
                        std::process::exit(1);
                    }
                }
            };
            let mut fs = FileInputStream::new(
                in_reader,
                config.input_skip_bits,
                config.input_skip_units,
                config.input_gap,
                config.input_assert_aligned,
                config.input_use_seek,
                config.input_reverse_bytes,
                config.input_reverse_unit,
                counter,
            );
            fs.unit_size = in_unit_size;
            fs.do_skip();
            Box::new(fs)
        };

        while let Some(unit) = unit_stream.next_unit() {
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
                    p.pack(tuple)
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
                let m_unit = ms.unit_size;
                match ms.next_unit() {
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

    // Flush sinks
    if let Some(ref mut tout) = tuple_direct_out {
        tout.flush_stream()?;
    }
    if let Some(ref mut sink) = unit_sink {
        sink.flush_stream()?;
    }

    Ok(())
}
