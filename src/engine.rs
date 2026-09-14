use crate::bits::BitValue;
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
    open_rewindable_file, BddReader, CounterStream, FileInputStream, IntegerInputStream, OneStream,
    PaddedUnitStream, RandomStream, RewindableBufRead, StreamConfig, StreamSeekBufReader,
    TupleDirectInput, UnitStream, ZeroStream,
};
use num_bigint::BigUint;
use num_traits::Zero;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};

/// Construct a UnitStream from a parsed StreamSpec and source name.
pub fn create_unit_stream_from_spec(
    source_name: &str,
    spec: &crate::stream_pattern::StreamSpec,
    default_unit: usize,
    seek_allowed: bool,
    use_mmap: bool,
) -> Result<Box<dyn UnitStream>, BddError> {
    let u_size = spec.unit_size.unwrap_or(default_unit);
    let stream: Box<dyn UnitStream> = match source_name {
        "zeros" => Box::new(ZeroStream::new_with_unit(Counter::new(0, None), u_size)),
        "ones" => Box::new(OneStream::new(Counter::new(0, None), u_size)),
        "rand" | "random" => Box::new(RandomStream::new(Counter::new(0, None), u_size)),
        "counter" => Box::new(CounterStream::new(Counter::new(0, None), u_size)),
        "stdin" | "-" => {
            let reader = BddReader::from_stdin(seek_allowed);
            let stream_conf = StreamConfig {
                skip_bits: spec.skip.unwrap_or(0),
                skip_units: 0,
                gap: spec.gap.unwrap_or(0),
                assert_aligned: false,
                drop_partial_eof: false,
                reverse_bytes: false,
                reverse_unit: false,
                unit_size: u_size,
                seek_allowed,
                repeat_count: 1,
            };
            let mut fs = FileInputStream::new(reader, stream_conf, Counter::new(0, None));
            fs.do_skip();
            Box::new(fs)
        }
        path => {
            let reader = match File::open(path) {
                Ok(f) => BddReader::from_file_with_mmap(f, seek_allowed, use_mmap),
                Err(_) => return Err(BddError::CannotOpenMergeFile(path.to_string())),
            };
            let stream_conf = StreamConfig {
                skip_bits: spec.skip.unwrap_or(0),
                skip_units: 0,
                gap: spec.gap.unwrap_or(0),
                assert_aligned: false,
                drop_partial_eof: false,
                reverse_bytes: false,
                reverse_unit: false,
                unit_size: u_size,
                seek_allowed,
                repeat_count: 1,
            };
            let mut fs = FileInputStream::new(reader, stream_conf, Counter::new(0, None));
            fs.do_skip();
            Box::new(fs)
        }
    };

    if spec.pad_zeros {
        Ok(Box::new(PaddedUnitStream::new(stream, true)))
    } else {
        Ok(stream)
    }
}

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

fn apply_manipulators(
    tuple: Vec<Field>,
    manipulators: &[Box<dyn TupleManipulator>],
) -> Option<Vec<Field>> {
    let mut current = Some(tuple);
    for m in manipulators {
        if let Some(t) = current {
            current = m.manipulate(t);
        } else {
            return None;
        }
    }
    current
}

fn dispatch_demux_sinks(
    demux_sinks: &mut [(usize, Box<dyn UnitSink>)],
    tuple: &[Field],
    unpacker: Option<&TupleUnpacker>,
    default_unit_size: usize,
) -> Result<(), BddError> {
    for (f_idx, sink) in demux_sinks.iter_mut() {
        if *f_idx < tuple.len() {
            let val = tuple[*f_idx].as_bit_value();
            let bits = if let Some(u) = unpacker {
                if *f_idx < u.pattern_items.len() {
                    u.pattern_items[*f_idx].bits
                } else {
                    default_unit_size
                }
            } else {
                default_unit_size
            };
            sink.write_bit_value(val, bits)?;
        }
    }
    Ok(())
}

fn resolve_output_unit(
    tuple: Vec<Field>,
    packer: Option<&TuplePacker>,
    unpacker: Option<&TupleUnpacker>,
    out_unit_size: usize,
    has_output_unit: bool,
) -> Result<(BitValue, usize), BddError> {
    if let Some(p) = packer {
        Ok((p.pack_bit_value(tuple)?, out_unit_size))
    } else if tuple.len() == 1 {
        let bits = match &tuple[0] {
            Field::Bits(_, b) => *b,
            _ => out_unit_size,
        };
        let eff_size = if has_output_unit { out_unit_size } else { bits };
        Ok((tuple[0].as_bit_value(), eff_size))
    } else if !tuple.is_empty() {
        let mut acc = BitValue::Inline(0);
        let mut total_bits = 0usize;
        for (i, f) in tuple.iter().enumerate() {
            let w = match f {
                Field::Bits(_, b) => *b,
                Field::Bytes(bytes) => bytes.len() * 8,
                _ => unpacker
                    .and_then(|u| u.field_widths().get(i).copied())
                    .unwrap_or(8),
            };
            acc = (acc << w) | f.as_bit_value();
            total_bits += w;
        }
        let eff_size = if has_output_unit {
            out_unit_size
        } else {
            total_bits
        };
        Ok((acc, eff_size))
    } else {
        crate::diag::warn("No fields to output, assumed 0");
        Ok((BitValue::Inline(0), out_unit_size))
    }
}

fn drain_merge_streams(
    merge_streams: &mut [Box<dyn UnitStream>],
    mut unit_sink: Option<&mut Box<dyn UnitSink>>,
) -> Result<bool, BddError> {
    for ms in merge_streams {
        let m_unit = ms.unit_size();
        match ms.next_bit_value()? {
            Some(u) => {
                if let Some(sink) = unit_sink.as_deref_mut() {
                    sink.write_bit_value(u, m_unit)?;
                }
            }
            None => {
                crate::diag::warn("Premature end of merge file");
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn run_pipeline_internal(
    config: ValidatedConfig,
    custom_writer: Option<Box<dyn Write>>,
) -> Result<(), BddError> {
    crate::diag::set_quiet(config.quiet);
    crate::diag::reset();
    let _diag_guard = DiagnosticGuard;

    if config.probe_units || config.probe_keys.is_some() {
        return run_unit_probe_internal(&config, custom_writer);
    }

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

    let mut merge_streams: Vec<Box<dyn UnitStream>> = Vec::new();
    if !config.merge_specs.is_empty() {
        for spec in &config.merge_specs {
            let src = spec.source.as_deref().unwrap_or("stdin");
            let ms = create_unit_stream_from_spec(
                src,
                spec,
                in_unit_size,
                config.merge_use_seek,
                config.merge_use_mmap,
            )?;
            merge_streams.push(ms);
        }
    } else {
        for mfile in &config.merge_files {
            let reader = if mfile == "-" {
                BddReader::from_stdin(config.merge_use_seek)
            } else {
                match File::open(mfile) {
                    Ok(f) => BddReader::from_file_with_mmap(
                        f,
                        config.merge_use_seek,
                        config.merge_use_mmap,
                    ),
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
            merge_streams.push(Box::new(ms));
        }
    }

    let field_names = unpacker.as_ref().and_then(|u| u.field_names());
    let field_widths = unpacker
        .as_ref()
        .map(|u| u.field_widths())
        .or_else(|| Some(vec![in_unit_size]));

    let mut manipulators: Vec<Box<dyn TupleManipulator>> = if !config.raw_args.is_empty() {
        build_pipeline_from_args_with_schema(
            &config.raw_args,
            field_names.as_deref(),
            field_widths.as_deref(),
        )?
    } else {
        Vec::new()
    };

    if manipulators.is_empty() {
        if let Some(ref arg) = config.rearrange {
            manipulators.push(Box::new(RearrangeManipulator::new_with_schema(
                arg,
                field_names.as_deref(),
                field_widths.as_deref(),
            )?));
        }
        if let Some(ref arg) = config.clamp {
            manipulators.push(Box::new(ClampManipulator::new(arg)?));
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
        if let Some(ref arg) = config.set {
            manipulators.push(Box::new(SetManipulator::new_with_schema(
                arg,
                field_names.as_deref(),
            )?));
        }
    }

    for m_spec in &config.inline_manipulators {
        manipulators.push(build_manipulator_from_spec_with_schema(
            m_spec,
            field_names.as_deref(),
            field_widths.as_deref(),
        )?);
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
        unit_sink = Some(Box::new(IntegerOutputStream::new(
            out_writer,
            config.output_reverse_bytes,
            config.output_reverse_unit,
        )));
    } else if config.output_hex {
        unit_sink = Some(Box::new(HexOutputStream::new(
            out_writer,
            config.output_reverse_bytes,
            config.output_reverse_unit,
        )));
    } else if config.output_bits {
        unit_sink = Some(Box::new(BitOutputStream::new(
            out_writer,
            config.output_reverse_bytes,
            config.output_reverse_unit,
        )));
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
            let bits = ms.read_bits(config.merge_copy_first as usize)?;
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
                Ok(f) => open_rewindable_file(f, config.input_use_mmap),
                Err(_) => return Err(BddError::CannotOpenInputFile(config.input_file.clone())),
            }
        };
        let mut tuple_in = TupleDirectInput::new(in_reader, counter, config.input_repeat);

        while let Some(tuple) = tuple_in.next_tuple()? {
            let tuple = match apply_manipulators(tuple, &manipulators) {
                Some(t) => t,
                None => continue,
            };

            dispatch_demux_sinks(&mut demux_sinks, &tuple, unpacker.as_ref(), out_unit_size)?;

            if let Some(ref mut tout) = tuple_sink {
                tout.write_tuple(&tuple)?;
            } else {
                let (unit, eff_unit_size) = resolve_output_unit(
                    tuple,
                    packer.as_ref(),
                    unpacker.as_ref(),
                    out_unit_size,
                    config.output_unit.is_some(),
                )?;
                if let Some(ref mut sink) = unit_sink {
                    write_framed_bit_value(
                        sink.as_mut(),
                        unit,
                        eff_unit_size,
                        config.output_raw_unit,
                        config.output_post_gap,
                        config.output_gap,
                        config.output_skip_bits,
                        &mut prefix_written,
                    )?;
                }
            }

            if drain_merge_streams(&mut merge_streams, unit_sink.as_mut())? {
                break;
            }
        }
    } else if config.overwrite && config.input_raw_unit.is_some() {
        let raw_bits = config.input_raw_unit.unwrap() as usize;
        let offset_bits = config.input_offset as usize;
        let slice_bits = in_unit_size;

        let mut raw_config = config.clone();
        raw_config.input_raw_unit = None;
        raw_config.input_offset = 0;
        raw_config.input_skip_bits = 0;
        raw_config.input_gap = 0;
        let mut container_stream = create_unit_stream(&raw_config, raw_bits, counter)?;

        // Overwrite mode: copy original initial skip bits to output as-is
        if config.input_skip_bits > 0 {
            let skip_bits = config.input_skip_bits as usize;
            let skip_data = container_stream.read_bits(skip_bits)?;
            if let Some(ref mut sink) = unit_sink {
                sink.write_bits(skip_data, skip_bits)?;
            }
        }

        while let Some(container) = container_stream.next_bit_value()? {
            let shift = raw_bits.saturating_sub(offset_bits + slice_bits);
            let unit = (container.clone() >> shift).mask_bits(slice_bits);

            let tuple = if let Some(ref u) = unpacker {
                u.unpack_bit_value(unit)
            } else {
                vec![Field::from(unit)]
            };

            let tuple = match apply_manipulators(tuple, &manipulators) {
                Some(t) => t,
                None => continue,
            };

            dispatch_demux_sinks(&mut demux_sinks, &tuple, unpacker.as_ref(), out_unit_size)?;

            let (out_unit, _) = resolve_output_unit(
                tuple,
                packer.as_ref(),
                unpacker.as_ref(),
                out_unit_size,
                false,
            )?;

            let mask = BitValue::mask_for(slice_bits);
            let full_mask = BitValue::mask_for(raw_bits);
            let clear_mask = full_mask ^ (mask.clone() << shift);
            let updated_container = (container & clear_mask) | ((out_unit & mask) << shift);

            if let Some(ref mut sink) = unit_sink {
                sink.write_bit_value(updated_container, raw_bits)?;
            }

            // Overwrite mode: copy original periodic gap bits to output as-is
            if config.input_gap > 0 {
                let gap_bits = config.input_gap as usize;
                let gap_data = container_stream.read_bits(gap_bits)?;
                if let Some(ref mut sink) = unit_sink {
                    sink.write_bits(gap_data, gap_bits)?;
                }
            }

            if drain_merge_streams(&mut merge_streams, unit_sink.as_mut())? {
                break;
            }
        }
    } else {
        let mut unit_stream = create_unit_stream(&config, in_unit_size, counter)?;

        let passthrough_fast_path = unpacker.is_none()
            && manipulators.is_empty()
            && demux_sinks.is_empty()
            && tuple_sink.is_none()
            && packer.is_none()
            && merge_streams.is_empty();

        if passthrough_fast_path {
            while let Some(unit) = unit_stream.next_bit_value()? {
                if let Some(ref mut sink) = unit_sink {
                    write_framed_bit_value(
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
        } else {
            while let Some(unit) = unit_stream.next_bit_value()? {
                let tuple = if let Some(ref u) = unpacker {
                    u.unpack_bit_value(unit)
                } else {
                    vec![Field::from(unit)]
                };

                let tuple = match apply_manipulators(tuple, &manipulators) {
                    Some(t) => t,
                    None => continue,
                };

                dispatch_demux_sinks(&mut demux_sinks, &tuple, unpacker.as_ref(), out_unit_size)?;

                if let Some(ref mut tout) = tuple_sink {
                    tout.write_tuple(&tuple)?;
                } else {
                    let (out_unit, eff_unit_size) = resolve_output_unit(
                        tuple,
                        packer.as_ref(),
                        unpacker.as_ref(),
                        out_unit_size,
                        config.output_unit.is_some(),
                    )?;
                    if let Some(ref mut sink) = unit_sink {
                        write_framed_bit_value(
                            sink.as_mut(),
                            out_unit,
                            eff_unit_size,
                            config.output_raw_unit,
                            config.output_post_gap,
                            config.output_gap,
                            config.output_skip_bits,
                            &mut prefix_written,
                        )?;
                    }
                }

                if drain_merge_streams(&mut merge_streams, unit_sink.as_mut())? {
                    break;
                }
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
        sink.write_u64(0, chunk)?;
        bits -= chunk as u64;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_framed_bit_value(
    sink: &mut dyn UnitSink,
    unit: BitValue,
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
        let framed = unit.mask_bits(unit_size) << (post_gap as usize);
        sink.write_bit_value(framed, raw as usize)?;
    } else {
        sink.write_bit_value(unit, unit_size)?;
    }

    if gap > 0 {
        write_zero_bits(sink, gap)?;
    }

    Ok(())
}

#[allow(clippy::too_many_arguments, dead_code)]
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
    write_framed_bit_value(
        sink,
        BitValue::from(unit),
        unit_size,
        raw_unit,
        post_gap,
        gap,
        skip_bits,
        prefix_written,
    )
}

pub fn create_unit_stream(
    config: &ValidatedConfig,
    in_unit_size: usize,
    counter: Counter,
) -> Result<Box<dyn UnitStream>, BddError> {
    let unit_stream: Box<dyn UnitStream> = if config.input_zeros {
        Box::new(ZeroStream::new_with_unit(counter, in_unit_size))
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
                Ok(f) => open_rewindable_file(f, config.input_use_mmap),
                Err(_) => return Err(BddError::CannotOpenInputFile(config.input_file.clone())),
            }
        };
        Box::new(IntegerInputStream::new(
            in_reader,
            counter,
            config.input_repeat,
        ))
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
                Ok(f) => {
                    BddReader::from_file_with_mmap(f, config.input_use_seek, config.input_use_mmap)
                }
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
    Ok(unit_stream)
}

fn run_unit_probe_internal(
    config: &ValidatedConfig,
    mut custom_writer: Option<Box<dyn Write>>,
) -> Result<(), BddError> {
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

    let key_search_bits = if let Some(ref ks) = config.probe_keys {
        Some(crate::probe::parse_key_bits(ks)?)
    } else {
        Some(256)
    };

    const MAX_PROBE_UNITS: usize = 1_048_576;
    let mut units = Vec::new();
    let target_unit_bits: usize;
    let target_description: String;

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
                Ok(f) => open_rewindable_file(f, config.input_use_mmap),
                Err(_) => return Err(BddError::CannotOpenInputFile(config.input_file.clone())),
            }
        };
        let mut tuple_in = TupleDirectInput::new(in_reader, counter, config.input_repeat);
        let field_idx = config.probe_field.unwrap_or(0);
        target_unit_bits = in_unit_size;
        target_description = format!("{} (tuples, field #{})", config.input_file, field_idx);

        while let Some(tuple) = tuple_in.next_tuple()? {
            if field_idx < tuple.len() {
                units.push(tuple[field_idx].as_biguint());
            } else if !tuple.is_empty() {
                units.push(tuple[0].as_biguint());
            } else {
                units.push(BigUint::zero());
            }
            if config.count.is_none() && units.len() >= MAX_PROBE_UNITS {
                break;
            }
        }
    } else {
        let mut unit_stream = create_unit_stream(config, in_unit_size, counter)?;

        if let Some(ref u) = unpacker {
            let field_idx = config.probe_field.unwrap_or(0);
            if field_idx >= u.pattern_items.len() {
                return Err(BddError::CliError(format!(
                    "Field index {} out of range for pattern with {} fields",
                    field_idx,
                    u.pattern_items.len()
                )));
            }
            let item = &u.pattern_items[field_idx];
            target_unit_bits = item.bits;
            target_description = format!(
                "{} [pattern field #{}: {}{}]",
                config.input_file, field_idx, item.bits, item.char_code
            );

            while let Some(raw_unit) = unit_stream.next_unit()? {
                let tuple = u.unpack(raw_unit);
                if field_idx < tuple.len() {
                    units.push(tuple[field_idx].as_biguint());
                }
                if config.count.is_none() && units.len() >= MAX_PROBE_UNITS {
                    break;
                }
            }
        } else {
            target_unit_bits = in_unit_size;
            let source_name = if config.input_file == "-" {
                if config.input_counter {
                    "counter stream".to_string()
                } else if config.input_random {
                    "random stream".to_string()
                } else if config.input_zeros {
                    "zero stream".to_string()
                } else if config.input_ones {
                    "ones stream".to_string()
                } else {
                    "stdin".to_string()
                }
            } else {
                config.input_file.clone()
            };
            target_description = format!("{} (unit size: {} bits)", source_name, target_unit_bits);

            while let Some(unit) = unit_stream.next_unit()? {
                units.push(unit);
                if config.count.is_none() && units.len() >= MAX_PROBE_UNITS {
                    break;
                }
            }
        }
    }

    let report = crate::probe::probe_unit_stream(
        &units,
        target_unit_bits,
        key_search_bits,
        &target_description,
    );

    let out_str = if config.output_json {
        crate::probe::format_unit_probe_json(&report)
    } else {
        crate::probe::format_unit_probe_text(&report)
    };

    if let Some(ref mut w) = custom_writer {
        w.write_all(out_str.as_bytes())?;
        w.write_all(b"\n")?;
    } else {
        println!("{}", out_str);
    }

    Ok(())
}
