use std::fs::File;
use std::io::Write;
use std::process::Command;

#[test]
fn test_integration_script() {
    // Ensure binary is built
    let build_status = Command::new("cargo")
        .args(["build", "--release"])
        .status()
        .expect("failed to run cargo build --release");
    assert!(build_status.success(), "cargo build --release failed");

    // Ensure ./bdd symlink exists
    let symlink_status = Command::new("ln")
        .args(["-sf", "target/release/bdd", "./bdd"])
        .status()
        .expect("failed to create bdd symlink");
    assert!(symlink_status.success(), "failed to create bdd symlink");

    // Execute test/test.sh
    let test_output = Command::new("bash")
        .arg("test/test.sh")
        .output()
        .expect("failed to execute test/test.sh");

    assert!(
        test_output.status.success(),
        "test.sh exited with status: {:?}\nstderr: {}",
        test_output.status,
        String::from_utf8_lossy(&test_output.stderr)
    );
}

#[test]
fn test_cli_large_unit_counter_hex() {
    let output = Command::new("./bdd")
        .args([
            "--input-counter",
            "--count=3",
            "--input-unit=256",
            "--output-unit=256",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd counter");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let tokens: Vec<&str> = stdout.split_whitespace().collect();
    assert_eq!(tokens.len(), 3);

    // 256 bits = 64 hex digits
    assert_eq!(tokens[0].len(), 64);
    assert_eq!(tokens[0], "0".repeat(64));
    assert_eq!(tokens[1], format!("{}1", "0".repeat(63)));
    assert_eq!(tokens[2], format!("{}2", "0".repeat(63)));
}

#[test]
fn test_cli_large_unit_ones_and_shift() {
    // Generate 1024 bits of all 1s as hex
    let output_ones = Command::new("./bdd")
        .args([
            "--input-ones",
            "--count=1",
            "--input-unit=1024",
            "--output-unit=1024",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd ones");

    assert!(output_ones.status.success());
    let stdout_ones = String::from_utf8(output_ones.stdout).unwrap();
    let hex_ones = stdout_ones.trim();
    // 1024 bits = 256 hex digits of 'f'
    assert_eq!(hex_ones.len(), 256);
    assert_eq!(hex_ones, "f".repeat(256));

    // Test XOR with 1024-bit mask (all bits become 0)
    let output_xor = Command::new("./bdd")
        .args([
            "--input-ones",
            "--count=1",
            "--input-unit=1024",
            "--xor=0,1024",
            "--output-unit=1024",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd xor");

    assert!(output_xor.status.success());
    let stdout_xor = String::from_utf8(output_xor.stdout).unwrap();
    let hex_xor = stdout_xor.trim();
    assert_eq!(hex_xor, "0".repeat(256));
}

#[test]
fn test_cli_large_tuple_rearrange() {
    // Pattern with 200 fields of 4-bit unsigned integers: 200 * 4 = 800 bits
    let pattern = "4U".repeat(200);
    // Rearrange: swap field 0 and field 199 (-1), and drop everything else
    let output = Command::new("./bdd")
        .args([
            "--input-ones",
            "--count=1",
            &format!("--input-pattern={}", pattern),
            "--rearrange=-1,0",
            "--output-tuples",
        ])
        .output()
        .expect("failed to run bdd large tuple");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // 4U ones = 15 (0xF)
    assert_eq!(stdout.trim(), "15,15");
}

#[test]
fn test_cli_auto_seek_and_no_seek() {
    use std::fs::File;
    use std::io::Write;

    let test_path = "/tmp/bdd_seek_test.bin";
    {
        let mut f = File::create(test_path).expect("failed to create test file");
        // Write 100,000 bytes with pattern
        let mut data = vec![0xAAu8; 99990];
        data.extend_from_slice(&[0x12, 0x34, 0x56, 0x78, 0x9A]);
        data.resize(100_000, 0x00);
        f.write_all(&data).expect("failed to write data");
    }

    // 1. Regular file with default auto-seeking
    let out_auto = Command::new("./bdd")
        .args([
            &format!("--input-file={}", test_path),
            "--input-skip-units=99990",
            "--count=5",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd auto seek");
    assert!(out_auto.status.success());
    let hex_auto = String::from_utf8(out_auto.stdout).unwrap();
    assert_eq!(hex_auto.trim(), "12 34 56 78 9a");

    // 2. Regular file with explicit --no-seek (streaming read fallback)
    let out_no_seek = Command::new("./bdd")
        .args([
            &format!("--input-file={}", test_path),
            "--input-skip-units=99990",
            "--count=5",
            "--no-seek",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd --no-seek");
    assert!(out_no_seek.status.success());
    let hex_no_seek = String::from_utf8(out_no_seek.stdout).unwrap();
    assert_eq!(hex_no_seek.trim(), "12 34 56 78 9a");

    // 3. Regular file with --do-not-seek alias
    let out_do_not_seek = Command::new("./bdd")
        .args([
            &format!("--input-file={}", test_path),
            "--input-skip-units=99990",
            "--count=5",
            "--do-not-seek",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd --do-not-seek");
    assert!(out_do_not_seek.status.success());
    let hex_do_not_seek = String::from_utf8(out_do_not_seek.stdout).unwrap();
    assert_eq!(hex_do_not_seek.trim(), "12 34 56 78 9a");

    // 4. Pipe via stdin (non-seekable stream fallback)
    let pipe_cmd = format!(
        "cat {} | ./bdd --input-skip-units=99990 --count=5 --output-hex",
        test_path
    );
    let out_pipe = Command::new("bash")
        .args(["-c", &pipe_cmd])
        .output()
        .expect("failed to run piped bdd");
    assert!(out_pipe.status.success());
    let hex_pipe = String::from_utf8(out_pipe.stdout).unwrap();
    assert_eq!(hex_pipe.trim(), "12 34 56 78 9a");

    let _ = std::fs::remove_file(test_path);
}

#[test]
fn test_cli_raw_unit_and_offset() {
    use std::fs::File;
    use std::io::Write;

    let test_path = "/tmp/bdd_raw_unit_test.bin";
    {
        let mut f = File::create(test_path).expect("failed to create test file");
        // Byte 0: 0b00110000 = 0x30 (bits 3,4 are '11' = 3; bit 5 is '0')
        // Byte 1: 0b00111000 = 0x38 (bits 3,4 are '11' = 3; bit 5 is '1')
        f.write_all(&[0x30, 0x38])
            .expect("failed to write test bytes");
    }

    // Extraction A: bits 3 and 4 of every 8-bit byte (offset 2, unit 2)
    let out_a = Command::new("./bdd")
        .args([
            &format!("--input-file={}", test_path),
            "--input-raw-unit=8",
            "--input-offset=2",
            "--input-unit=2",
            "--output-integers",
        ])
        .output()
        .expect("failed to run bdd raw unit A");
    assert!(out_a.status.success());
    let stdout_a = String::from_utf8(out_a.stdout).unwrap();
    let lines_a: Vec<&str> = stdout_a.lines().collect();
    assert_eq!(lines_a, vec!["3", "3"]);

    // Extraction B: bit 5 of every 8-bit byte (offset 4, unit 1)
    let out_b = Command::new("./bdd")
        .args([
            &format!("--input-file={}", test_path),
            "--input-raw-unit=8",
            "--input-offset=4",
            "--input-unit=1",
            "--output-integers",
        ])
        .output()
        .expect("failed to run bdd raw unit B");
    assert!(out_b.status.success());
    let stdout_b = String::from_utf8(out_b.stdout).unwrap();
    let lines_b: Vec<&str> = stdout_b.lines().collect();
    assert_eq!(lines_b, vec!["0", "1"]);

    let _ = std::fs::remove_file(test_path);
}

#[test]
fn test_cli_gigabyte_seek_and_suffixes() {
    use std::io::Seek;
    let test_path = "/tmp/bdd_gigabyte_test.bin";
    {
        let mut f = File::create(test_path).expect("failed to create 10GB test file");
        // Create 10 GiB sparse file
        f.set_len(10 * 1024 * 1024 * 1024 + 4)
            .expect("failed to set len");
        f.seek(std::io::SeekFrom::Start(10 * 1024 * 1024 * 1024))
            .expect("failed to seek in test file");
        f.write_all(&[0xDE, 0xAD, 0xBE, 0xEF])
            .expect("failed to write magic bytes at 10GiB offset");
    }

    // Seek directly over 10GiB in O(1) time using size suffix
    let out = Command::new("./bdd")
        .args([
            &format!("--input-file={}", test_path),
            "--input-skip-bits=10GiB",
            "--count=4",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd gigabyte seek");

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "de ad be ef");

    // Also test unit-based skipping over 10GiB
    let out_units = Command::new("./bdd")
        .args([
            &format!("--input-file={}", test_path),
            "--input-unit=8",
            "--input-skip-units=10Gi",
            "--count=4",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd gigabyte skip-units");

    assert!(out_units.status.success());
    let stdout_units = String::from_utf8(out_units.stdout).unwrap();
    assert_eq!(stdout_units.trim(), "de ad be ef");

    let _ = std::fs::remove_file(test_path);
}

#[test]
fn test_cli_round_and_float_transcoding() {
    use std::io::Write;

    // 1. Test --round with mode-only (floor, round_ties_even)
    let out_round = Command::new("./bdd")
        .args(["--input-tuples", "--round=0,floor", "--output-tuples"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(b"2.9\n").unwrap();
            child.wait_with_output()
        })
        .expect("failed to run bdd --round 0,floor");

    assert!(out_round.status.success());
    let stdout_round = String::from_utf8(out_round.stdout).unwrap();
    assert_eq!(stdout_round.trim(), "2");

    // 2. Test --round alias --cut-maxint
    let out_cut = Command::new("./bdd")
        .args([
            "--input-tuples",
            "--cut-maxint=0,10,saturate",
            "--output-tuples",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(b"42\n").unwrap();
            child.wait_with_output()
        })
        .expect("failed to run bdd --cut-maxint");

    assert!(out_cut.status.success());
    let stdout_cut = String::from_utf8(out_cut.stdout).unwrap();
    assert_eq!(stdout_cut.trim(), "10");

    // 3. Test cross-precision transcoding: FP32 (32F) -> FP16 (16H)
    // 1.0f32 big-endian is 0x3F800000; 1.0 in FP16 is 0x3C00
    let out_transcode = Command::new("./bdd")
        .args([
            "--input-pattern=32F",
            "--output-pattern=16H",
            "--output-hex",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(&[0x3F, 0x80, 0x00, 0x00])
                .unwrap();
            child.wait_with_output()
        })
        .expect("failed to run bdd transcode 32F -> 16H");

    assert!(out_transcode.status.success());
    let hex_transcode = String::from_utf8(out_transcode.stdout).unwrap();
    assert_eq!(hex_transcode.trim(), "3c00");

    // 4. Test cross-precision transcoding: FP16 (16H) -> FP8 E4M3 (8E)
    // 1.0 in FP16 is 0x3C00; 1.0 in FP8 E4M3 is 0x38
    let out_fp8 = Command::new("./bdd")
        .args(["--input-pattern=16H", "--output-pattern=8E", "--output-hex"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(&[0x3C, 0x00])
                .unwrap();
            child.wait_with_output()
        })
        .expect("failed to run bdd transcode 16H -> 8E");

    assert!(out_fp8.status.success());
    let hex_fp8 = String::from_utf8(out_fp8.stdout).unwrap();
    assert_eq!(hex_fp8.trim(), "38");
}

#[test]
fn test_cli_multiplication_sizes() {
    use std::fs::File;
    use std::io::{Seek, Write};
    let test_path = "/tmp/bdd_mul_test.bin";
    {
        let mut f = File::create(test_path).expect("failed to create mul test file");
        // 1,000,000 * 24 bits = 24,000,000 bits = 3,000,000 bytes
        let target_byte_offset = 1_000_000 * 3; // 3,000,000 bytes
        f.set_len(target_byte_offset + 8)
            .expect("failed to set len");
        f.seek(std::io::SeekFrom::Start(target_byte_offset))
            .expect("failed to seek");
        f.write_all(&[0x11, 0x22, 0x33, 0x44])
            .expect("failed to write magic bytes");
    }

    // Skip 1,000,000 24-bit units using 1000000*24
    let out = Command::new("./bdd")
        .args([
            &format!("--input-file={}", test_path),
            "--input-skip-bits=1000000*24",
            "--count=4",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd multiplication skip");

    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "11 22 33 44");

    // Also test chained multiplication and count
    let out_chained = Command::new("./bdd")
        .args([
            &format!("--input-file={}", test_path),
            "--input-skip-bits=1000*1000*24",
            "--count=2*2",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd chained multiplication");

    assert!(out_chained.status.success());
    let stdout_chained = String::from_utf8(out_chained.stdout).unwrap();
    assert_eq!(stdout_chained.trim(), "11 22 33 44");

    let _ = std::fs::remove_file(test_path);
}
