use crate::cli::ValidatedConfig;
use crate::counter::Counter;
use crate::error::BddError;
use crate::field::Field;
use crate::manipulator::*;
use crate::pattern::{TuplePacker, TupleUnpacker};
use crate::sink::{
    BitOutputStream, CsvOutputStream, FileOutputStream, HexOutputStream, IntegerOutputStream,
    JsonOutputStream, TupleDirectOutput, TupleSink, UnitSink, VisualOutputStream,
};
use crate::stream::{
    BddReader, CounterStream, FileInputStream, IntegerInputStream, OneStream, RandomStream,
    RewindableBufRead, StreamConfig, StreamSeekBufReader, TupleDirectInput, UnitStream, ZeroStream,
};
use num_bigint::BigUint;
use num_traits::Zero;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};

/// Runs the complete bdd pipeline based on validated configuration.
pub fn run_pipeline(config: ValidatedConfig) -> Result<(), BddError> {
    run_pipeline_internal(config, None)
}

/// Runs the complete bdd pipeline capturing output into a custom writer.
pub fn run_pipeline_to_writer<W: Write + 'static>(
    config: ValidatedConfig,
    writer: W,
) -> Result<(), BddError> {
    run_pipeline_internal(config, Some(Box::new(writer)))
}

struct DiagnosticGuard;

impl Drop for DiagnosticGuard {
    fn drop(&mut self) {
        crate::diag::flush_summary();
    }
}

fn run_pipeline_internal(
    config: ValidatedConfig,
    custom_writer: Option<Box<dyn Write>>,
) -> Result<(), BddError> {
    crate::diag::set_quiet(config.quiet);
    crate::diag::reset();
    let _diag_guard = DiagnosticGuard;

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

    let mut merge_streams: Vec<FileInputStream<BddReader>> = Vec::new();
    for mfile in &config.merge_files {
        let reader = if mfile == "-" {
            BddReader::from_stdin(config.merge_use_seek)
        } else {
            match File::open(mfile) {
                Ok(f) => BddReader::from_file(f, config.merge_use_seek),
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
            drop_partial_eof: config.merge_drop_partial_eof,
            reverse_bytes: config.merge_reverse_bytes,
            reverse_unit: config.merge_reverse_unit,
            unit_size: m_unit,
            seek_allowed: config.merge_use_seek,
            repeat_count: 1,
        };
        let mut ms = FileInputStream::new(reader, stream_conf, Counter::new(0, None));
        ms.do_skip();
        merge_streams.push(ms);
    }

    // Build manipulation pipeline: ordered from CLI arguments if available, else from flags
    let mut manipulators: Vec<Box<dyn TupleManipulator>> = if !config.raw_args.is_empty() {
        build_pipeline_from_args(&config.raw_args)?
    } else {
        Vec::new()
    };

    if manipulators.is_empty() {
        if let Some(ref arg) = config.rearrange {
            manipulators.push(Box::new(RearrangeManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.round {
            manipulators.push(Box::new(RoundManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.remove_right {
            manipulators.push(Box::new(RemoveRightManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.shift_right {
            manipulators.push(Box::new(ShiftRightManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.shift_left {
            manipulators.push(Box::new(ShiftLeftManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.xor {
            manipulators.push(Box::new(XorManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.and {
            manipulators.push(Box::new(AndManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.or {
            manipulators.push(Box::new(OrManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.not {
            manipulators.push(Box::new(NotManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.abs {
            manipulators.push(Box::new(AbsManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.sign {
            manipulators.push(Box::new(SignManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.add {
            manipulators.push(Box::new(AddManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.sub {
            manipulators.push(Box::new(SubManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.mul {
            manipulators.push(Box::new(MulManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.div {
            manipulators.push(Box::new(DivManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.r#mod {
            manipulators.push(Box::new(ModManipulator::new(arg)?));
        }
        if let Some(ref arg) = config.filter {
            manipulators.push(Box::new(FilterManipulator::new(arg)?));
        }
    }

    let out_writer: Box<dyn Write> = if let Some(w) = custom_writer {
        w
    } else if config.output_file == "-" {
        Box::new(io::stdout())
    } else {
        match File::create(&config.output_file) {
            Ok(f) => Box::new(BufWriter::new(f)),
            Err(_) => return Err(BddError::CannotOpenOutputFile),
        }
    };

    let mut tuple_sink: Option<Box<dyn TupleSink>> = None;
    let mut unit_sink: Option<Box<dyn UnitSink>> = None;

    if config.output_tuples {
        tuple_sink = Some(Box::new(TupleDirectOutput::new(out_writer)));
    } else if config.output_json {
        let field_names = if let Some(ref jf) = config.json_fields {
            Some(jf.clone())
        } else if let Some(ref u) = unpacker {
            u.field_names()
        } else if config.json_object {
            Some(Vec::new())
        } else {
            None
        };
        tuple_sink = Some(Box::new(JsonOutputStream::new(out_writer, field_names)));
    } else if config.output_csv {
        let header = config.csv_header.clone().or_else(|| {
            if let Some(ref jf) = config.json_fields {
                Some(jf.join(","))
            } else if let Some(ref u) = unpacker {
                u.field_names().map(|names| names.join(","))
            } else {
                None
            }
        });
        tuple_sink = Some(Box::new(CsvOutputStream::new(out_writer, header)));
    } else if config.output_visual {
        tuple_sink = Some(Box::new(VisualOutputStream::new(out_writer)));
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

    // Demux sinks
    let mut demux_sinks: Vec<(usize, Box<dyn UnitSink>)> = Vec::new();
    if let Some(ref df) = config.demux_files {
        for (i, path) in df.split(',').enumerate() {
            let p = path.trim();
            if !p.is_empty() {
                let w: Box<dyn Write> = if p == "-" {
                    Box::new(io::stdout())
                } else {
                    Box::new(BufWriter::new(
                        File::create(p).map_err(|_| BddError::CannotOpenOutputFile)?,
                    ))
                };
                demux_sinks.push((i, Box::new(FileOutputStream::new(w, false, false))));
            }
        }
    }
    for spec in &config.demux {
        let parts: Vec<&str> = spec.splitn(2, ':').collect();
        if parts.len() == 2 {
            let field_idx = parts[0].trim().parse::<usize>().unwrap_or(0);
            let p = parts[1].trim();
            let w: Box<dyn Write> = if p == "-" {
                Box::new(io::stdout())
            } else {
                Box::new(BufWriter::new(
                    File::create(p).map_err(|_| BddError::CannotOpenOutputFile)?,
                ))
            };
            demux_sinks.push((field_idx, Box::new(FileOutputStream::new(w, false, false))));
        }
    }

    if let Some(ms) = merge_streams.first_mut() {
        if config.merge_copy_first > 0 {
            let bits = ms.read_bits(config.merge_copy_first as usize);
            if let Some(ref mut sink) = unit_sink {
                sink.write_bits(bits, config.merge_copy_first as usize)?;
            }
        }
    }

    let mut prefix_written = false;
    let counter = Counter::new(config.skip, config.count);

    if config.input_tuples {
        let in_reader: Box<dyn RewindableBufRead> = if config.input_file == "-" {
            if config.input_repeat != 1 {
                let mut buf = Vec::new();
                io::stdin().read_to_end(&mut buf)?;
                Box::new(std::io::Cursor::new(buf))
            } else {
                Box::new(StreamSeekBufReader(BufReader::new(io::stdin())))
            }
        } else {
            match File::open(&config.input_file) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(_) => return Err(BddError::CannotOpenInputFile(config.input_file.clone())),
            }
        };
        let mut tuple_in = TupleDirectInput::new(in_reader, counter, config.input_repeat);

        while let Some(mut tuple) = tuple_in.next_tuple()? {
            let mut maybe_tuple = Some(tuple);
            for m in &manipulators {
                if let Some(t) = maybe_tuple {
                    maybe_tuple = m.manipulate(t);
                } else {
                    break;
                }
            }
            tuple = match maybe_tuple {
                Some(t) => t,
                None => continue,
            };

            for (f_idx, sink) in demux_sinks.iter_mut() {
                if *f_idx < tuple.len() {
                    let val = tuple[*f_idx].as_biguint();
                    let bits = if let Some(ref u) = unpacker {
                        if *f_idx < u.pattern_items.len() {
                            u.pattern_items[*f_idx].bits
                        } else {
                            out_unit_size
                        }
                    } else {
                        out_unit_size
                    };
                    sink.write_bits(val, bits)?;
                }
            }

            if let Some(ref mut tout) = tuple_sink {
                tout.write_tuple(&tuple)?;
            } else {
                let unit = if let Some(ref p) = packer {
                    p.pack(tuple)?
                } else if !tuple.is_empty() {
                    tuple[0].as_biguint()
                } else {
                    crate::diag::warn("No fields to output, assumed 0");
                    BigUint::zero()
                };
                if let Some(ref mut sink) = unit_sink {
                    write_framed_unit(
                        sink.as_mut(),
                        unit,
                        out_unit_size,
                        config.output_raw_unit,
                        config.output_post_gap,
                        config.output_gap,
                        config.output_skip_bits,
                        &mut prefix_written,
                    )?;
                }
            }
            let mut premature_eof = false;
            for ms in &mut merge_streams {
                let m_unit = ms.config.unit_size;
                match ms.next_unit()? {
                    Some(u) => {
                        if let Some(ref mut sink) = unit_sink {
                            sink.write_bits(u, m_unit)?;
                        }
                    }
                    None => {
                        crate::diag::warn("Premature end of merge file");
                        premature_eof = true;
                        break;
                    }
                }
            }
            if premature_eof {
                break;
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
            let in_reader: Box<dyn RewindableBufRead> = if config.input_file == "-" {
                if config.input_repeat != 1 {
                    let mut buf = Vec::new();
                    io::stdin().read_to_end(&mut buf)?;
                    Box::new(std::io::Cursor::new(buf))
                } else {
                    Box::new(StreamSeekBufReader(BufReader::new(io::stdin())))
                }
            } else {
                match File::open(&config.input_file) {
                    Ok(f) => Box::new(BufReader::new(f)),
                    Err(_) => return Err(BddError::CannotOpenInputFile(config.input_file.clone())),
                }
            };
            Box::new(IntegerInputStream::new(in_reader, counter, config.input_repeat))
        } else {
            let in_reader = if config.input_file == "-" {
                if config.input_repeat != 1 {
                    let mut buf = Vec::new();
                    io::stdin().read_to_end(&mut buf)?;
                    BddReader::new_seekable(std::io::Cursor::new(buf), true)
                } else {
                    BddReader::from_stdin(config.input_use_seek)
                }
            } else {
                match File::open(&config.input_file) {
                    Ok(f) => BddReader::from_file(f, config.input_use_seek),
                    Err(_) => return Err(BddError::CannotOpenInputFile(config.input_file.clone())),
                }
            };
            let stream_conf = StreamConfig {
                skip_bits: config.input_skip_bits,
                skip_units: config.input_skip_units,
                gap: config.input_gap,
                assert_aligned: config.input_assert_aligned,
                drop_partial_eof: config.input_drop_partial_eof,
                reverse_bytes: config.input_reverse_bytes,
                reverse_unit: config.input_reverse_unit,
                unit_size: in_unit_size,
                seek_allowed: config.input_use_seek,
                repeat_count: config.input_repeat,
            };
            let mut fs = FileInputStream::new(in_reader, stream_conf, counter);
            fs.do_skip();
            Box::new(fs)
        };

        while let Some(unit) = unit_stream.next_unit()? {
            let tuple = if let Some(ref u) = unpacker {
                u.unpack(unit)
            } else {
                vec![Field::UInt(unit)]
            };

            let mut maybe_tuple = Some(tuple);
            for m in &manipulators {
                if let Some(t) = maybe_tuple {
                    maybe_tuple = m.manipulate(t);
                } else {
                    break;
                }
            }
            let tuple = match maybe_tuple {
                Some(t) => t,
                None => continue,
            };

            for (f_idx, sink) in demux_sinks.iter_mut() {
                if *f_idx < tuple.len() {
                    let val = tuple[*f_idx].as_biguint();
                    let bits = if let Some(ref u) = unpacker {
                        if *f_idx < u.pattern_items.len() {
                            u.pattern_items[*f_idx].bits
                        } else {
                            out_unit_size
                        }
                    } else {
                        out_unit_size
                    };
                    sink.write_bits(val, bits)?;
                }
            }

            if let Some(ref mut tout) = tuple_sink {
                tout.write_tuple(&tuple)?;
            } else {
                let out_unit = if let Some(ref p) = packer {
                    p.pack(tuple)?
                } else if !tuple.is_empty() {
                    tuple[0].as_biguint()
                } else {
                    crate::diag::warn("No fields to output, assumed 0");
                    BigUint::zero()
                };
                if let Some(ref mut sink) = unit_sink {
                    write_framed_unit(
                        sink.as_mut(),
                        out_unit,
                        out_unit_size,
                        config.output_raw_unit,
                        config.output_post_gap,
                        config.output_gap,
                        config.output_skip_bits,
                        &mut prefix_written,
                    )?;
                }
            }

            let mut premature_eof = false;
            for ms in &mut merge_streams {
                let m_unit = ms.config.unit_size;
                match ms.next_unit()? {
                    Some(u) => {
                        if let Some(ref mut sink) = unit_sink {
                            sink.write_bits(u, m_unit)?;
                        }
                    }
                    None => {
                        crate::diag::warn("Premature end of merge file");
                        premature_eof = true;
                        break;
                    }
                }
            }
            if premature_eof {
                break;
            }
        }
    }

    if let Some(ref mut tout) = tuple_sink {
        tout.flush_stream()?;
    }
    if let Some(ref mut sink) = unit_sink {
        sink.flush_stream()?;
    }
    for (_, sink) in demux_sinks.iter_mut() {
        sink.flush_stream()?;
    }

    Ok(())
}

fn write_zero_bits(sink: &mut dyn UnitSink, mut bits: u64) -> Result<(), BddError> {
    while bits > 0 {
        let chunk = bits.min(64) as usize;
        sink.write_bits(num_bigint::BigUint::default(), chunk)?;
        bits -= chunk as u64;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_framed_unit(
    sink: &mut dyn UnitSink,
    unit: num_bigint::BigUint,
    unit_size: usize,
    raw_unit: Option<u64>,
    post_gap: u64,
    gap: u64,
    skip_bits: u64,
    prefix_written: &mut bool,
) -> Result<(), BddError> {
    if !*prefix_written {
        if skip_bits > 0 {
            write_zero_bits(sink, skip_bits)?;
        }
        *prefix_written = true;
    }

    if let Some(raw) = raw_unit {
        let mask = if unit_size > 0 {
            (num_bigint::BigUint::from(1u32) << unit_size) - 1u32
        } else {
            num_bigint::BigUint::default()
        };
        let framed = (unit & mask) << (post_gap as usize);
        sink.write_bits(framed, raw as usize)?;
    } else {
        sink.write_bits(unit, unit_size)?;
    }

    if gap > 0 {
        write_zero_bits(sink, gap)?;
    }

    Ok(())
}
