use bdd::counter::Counter;
use bdd::field::reverse_bits;
use bdd::manipulator::{RearrangeManipulator, TupleManipulator};
use bdd::pattern::{TuplePacker, TupleUnpacker};
use bdd::sink::{FileOutputStream, UnitSink};
use bdd::stream::{FileInputStream, StreamConfig, UnitStream, ZeroStream};
use num_bigint::BigUint;
use num_traits::One;
use std::io::Cursor;
use std::time::Instant;

fn format_rate(bits: u64, duration_secs: f64) -> String {
    let bps = (bits as f64) / duration_secs;
    if bps >= 1_000_000_000.0 {
        format!(
            "{:.2} Gbps ({:.2} MB/s)",
            bps / 1_000_000_000.0,
            bps / 8_000_000.0
        )
    } else if bps >= 1_000_000.0 {
        format!(
            "{:.2} Mbps ({:.2} MB/s)",
            bps / 1_000_000.0,
            bps / 8_000_000.0
        )
    } else if bps >= 1_000.0 {
        format!("{:.2} Kbps ({:.2} KB/s)", bps / 1_000.0, bps / 8_000.0)
    } else {
        format!("{:.2} bps", bps)
    }
}

fn bench_zero_stream_8bit() {
    let count = 2_000_000;
    let mut zs = ZeroStream::new(Counter::new(0, Some(count)));
    let mut out = Vec::with_capacity(count);
    let mut sink = FileOutputStream::new(&mut out, false, false);

    let start = Instant::now();
    while let Ok(Some(unit)) = zs.next_unit() {
        sink.write_bits(unit, 8).unwrap();
    }
    sink.flush_stream().unwrap();
    let duration = start.elapsed().as_secs_f64();
    let total_bits = (count as u64) * 8;

    println!(
        "| Synthetic 8-bit stream generation  | {:>10} bits | {:>8.3}s | {:>22} |",
        total_bits,
        duration,
        format_rate(total_bits, duration)
    );
}

fn bench_single_bit_streaming() {
    let count = 500_000;
    let mut zs = ZeroStream::new(Counter::new(0, Some(count)));
    let mut out = Vec::with_capacity(count / 8);
    let mut sink = FileOutputStream::new(&mut out, false, false);

    let start = Instant::now();
    while let Ok(Some(unit)) = zs.next_unit() {
        sink.write_bits(unit, 1).unwrap();
    }
    sink.flush_stream().unwrap();
    let duration = start.elapsed().as_secs_f64();
    let total_bits = count as u64;

    println!(
        "| 1-bit single-bit resolution stream | {:>10} bits | {:>8.3}s | {:>22} |",
        total_bits,
        duration,
        format_rate(total_bits, duration)
    );
}

fn bench_unaligned_3bit_to_8bit() {
    let count = 1_000_000;
    let raw_data = vec![0xAAu8; count * 3 / 8 + 10];
    let stream_conf = StreamConfig {
        skip_bits: 0,
        skip_units: 0,
        gap: 0,
        assert_aligned: false,
        reverse_bytes: false,
        reverse_unit: false,
        unit_size: 3,
    };
    let mut fs = FileInputStream::new(
        Cursor::new(raw_data),
        stream_conf,
        Counter::new(0, Some(count)),
    );
    let mut out = Vec::with_capacity(count);
    let mut sink = FileOutputStream::new(&mut out, false, false);

    let start = Instant::now();
    while let Ok(Some(unit)) = fs.next_unit() {
        sink.write_bits(unit, 8).unwrap();
    }
    sink.flush_stream().unwrap();
    let duration = start.elapsed().as_secs_f64();
    let total_bits = (count as u64) * 3;

    println!(
        "| Unaligned 3-bit unpack to 8-bit    | {:>10} bits | {:>8.3}s | {:>22} |",
        total_bits,
        duration,
        format_rate(total_bits, duration)
    );
}

fn bench_tuple_unpack_rearrange_pack() {
    let count = 250_000;
    let unpacker = TupleUnpacker::new("2U3U3U").unwrap();
    let packer = TuplePacker::new("3U2U3U").unwrap();
    let manip = RearrangeManipulator::new("1,0,2").unwrap();

    let input_unit = BigUint::from(0b10110111u32);

    let start = Instant::now();
    let mut total_packed = BigUint::from(0u32);
    for _ in 0..count {
        let tuple = unpacker.unpack(input_unit.clone());
        let modified = manip.manipulate(tuple).unwrap();
        let packed = packer.pack(modified).unwrap();
        total_packed += packed;
    }
    let duration = start.elapsed().as_secs_f64();
    let total_bits = (count as u64) * 8;
    assert!(total_packed > BigUint::from(0u32));

    println!(
        "| Tuple pipeline (2U3U3U -> rearrange)| {:>10} bits | {:>8.3}s | {:>22} |",
        total_bits,
        duration,
        format_rate(total_bits, duration)
    );
}

fn bench_bignum_256bit_units() {
    let count = 50_000;
    let unpacker = TupleUnpacker::new("256U").unwrap();
    let packer = TuplePacker::new("256U").unwrap();
    let val_256 = (BigUint::one() << 255usize) | BigUint::from(0xDEADBEEFu64);

    let start = Instant::now();
    for _ in 0..count {
        let tuple = unpacker.unpack(val_256.clone());
        let _packed = packer.pack(tuple).unwrap();
    }
    let duration = start.elapsed().as_secs_f64();
    let total_bits = (count as u64) * 256;

    println!(
        "| Bignum 256-bit unpack/pack         | {:>10} bits | {:>8.3}s | {:>22} |",
        total_bits,
        duration,
        format_rate(total_bits, duration)
    );
}

fn bench_bignum_1024bit_units() {
    let count = 10_000;
    let unpacker = TupleUnpacker::new("1024U").unwrap();
    let packer = TuplePacker::new("1024U").unwrap();
    let val_1024 =
        (BigUint::one() << 1023usize) | (BigUint::one() << 512usize) | BigUint::from(0x12345u32);

    let start = Instant::now();
    for _ in 0..count {
        let tuple = unpacker.unpack(val_1024.clone());
        let _packed = packer.pack(tuple).unwrap();
    }
    let duration = start.elapsed().as_secs_f64();
    let total_bits = (count as u64) * 1024;

    println!(
        "| Bignum 1024-bit unpack/pack        | {:>10} bits | {:>8.3}s | {:>22} |",
        total_bits,
        duration,
        format_rate(total_bits, duration)
    );
}

fn bench_hardware_vs_bignum_bit_reversal() {
    let count_64 = 2_000_000;
    let val_64 = BigUint::from(0x12345678_9ABCDEF0u64);
    let start = Instant::now();
    for _ in 0..count_64 {
        let _ = reverse_bits(&val_64, 64);
    }
    let dur_64 = start.elapsed().as_secs_f64();
    let bits_64 = (count_64 as u64) * 64;

    println!(
        "| Hardware 64-bit reversal (.reverse)| {:>10} bits | {:>8.3}s | {:>22} |",
        bits_64,
        dur_64,
        format_rate(bits_64, dur_64)
    );

    let count_1024 = 5_000;
    let val_1024 = (BigUint::one() << 1023usize) | BigUint::one();
    let start = Instant::now();
    for _ in 0..count_1024 {
        let _ = reverse_bits(&val_1024, 1024);
    }
    let dur_1024 = start.elapsed().as_secs_f64();
    let bits_1024 = (count_1024 as u64) * 1024;

    println!(
        "| Bignum 1024-bit bit reversal       | {:>10} bits | {:>8.3}s | {:>22} |",
        bits_1024,
        dur_1024,
        format_rate(bits_1024, dur_1024)
    );
}

fn main() {
    println!("\n=== bdd Performance Benchmarks ===");
    println!(
        "Platform: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!("Compiler: Rust 1.x (Release build with LTO)");
    println!();
    println!(
        "| Operation                          | Total Volume | Duration |             Throughput |"
    );
    println!(
        "|:-----------------------------------|-------------:|---------:|-----------------------:|"
    );

    bench_zero_stream_8bit();
    bench_single_bit_streaming();
    bench_unaligned_3bit_to_8bit();
    bench_tuple_unpack_rearrange_pack();
    bench_bignum_256bit_units();
    bench_bignum_1024bit_units();
    bench_hardware_vs_bignum_bit_reversal();

    println!();
}
