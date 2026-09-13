use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

const BDD_BIN: &str = env!("CARGO_BIN_EXE_bdd");

#[test]
fn test_integration_script() {
    // Ensure ./bdd symlink exists pointing to BDD_BIN so test.sh can invoke ./bdd
    let _ = std::fs::remove_file("./bdd");
    #[cfg(unix)]
    let _ = std::os::unix::fs::symlink(BDD_BIN, "./bdd");
    #[cfg(not(unix))]
    let _ = std::fs::copy(BDD_BIN, "./bdd");

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
    let output = Command::new(BDD_BIN)
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
    let output_ones = Command::new(BDD_BIN)
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
    let output_xor = Command::new(BDD_BIN)
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
    let output = Command::new(BDD_BIN)
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
    let out_auto = Command::new(BDD_BIN)
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
    let out_no_seek = Command::new(BDD_BIN)
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
    let out_do_not_seek = Command::new(BDD_BIN)
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
fn test_cli_mmap_and_no_mmap() {
    use std::fs::File;
    use std::io::Write;

    let test_path = "/tmp/bdd_mmap_test.bin";
    {
        let mut f = File::create(test_path).expect("failed to create test file");
        let data: Vec<u8> = (0..256).map(|i| i as u8).collect();
        f.write_all(&data).expect("failed to write data");
    }

    // 1. Default (mmap enabled on regular file)
    let out_default = Command::new(BDD_BIN)
        .args([
            &format!("--input-file={}", test_path),
            "--input-skip-units=10",
            "--count=5",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd default");
    assert!(out_default.status.success());
    let hex_default = String::from_utf8(out_default.stdout).unwrap();
    assert_eq!(hex_default.trim(), "0a 0b 0c 0d 0e");

    // 2. Explicit --mmap
    let out_mmap = Command::new(BDD_BIN)
        .args([
            &format!("--input-file={}", test_path),
            "--mmap",
            "--input-skip-units=10",
            "--count=5",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd --mmap");
    assert!(out_mmap.status.success());
    let hex_mmap = String::from_utf8(out_mmap.stdout).unwrap();
    assert_eq!(hex_mmap.trim(), "0a 0b 0c 0d 0e");

    // 3. Explicit --no-mmap (buffered I/O)
    let out_no_mmap = Command::new(BDD_BIN)
        .args([
            &format!("--input-file={}", test_path),
            "--no-mmap",
            "--input-skip-units=10",
            "--count=5",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd --no-mmap");
    assert!(out_no_mmap.status.success());
    let hex_no_mmap = String::from_utf8(out_no_mmap.stdout).unwrap();
    assert_eq!(hex_no_mmap.trim(), "0a 0b 0c 0d 0e");

    // 4. Probing with mmap
    let out_probe = Command::new(BDD_BIN)
        .args(["--probe", test_path, "--output-json"])
        .output()
        .expect("failed to probe with mmap");
    assert!(out_probe.status.success());
    let json_str = String::from_utf8(out_probe.stdout).unwrap();
    assert!(json_str.contains("\"sample_bytes\": 256"));

    // 5. Probing with --no-mmap
    let out_probe_no_mmap = Command::new(BDD_BIN)
        .args(["--no-mmap", "--probe", test_path, "--output-json"])
        .output()
        .expect("failed to probe with --no-mmap");
    assert!(out_probe_no_mmap.status.success());
    let json_str2 = String::from_utf8(out_probe_no_mmap.stdout).unwrap();
    assert!(json_str2.contains("\"sample_bytes\": 256"));

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
    let out_a = Command::new(BDD_BIN)
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
    let out_b = Command::new(BDD_BIN)
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
    let out = Command::new(BDD_BIN)
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
    let out_units = Command::new(BDD_BIN)
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
    let out_round = Command::new(BDD_BIN)
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
    let out_cut = Command::new(BDD_BIN)
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

    #[cfg(feature = "small-floats")]
    {
        // 3. Test cross-precision transcoding: FP32 (32F) -> FP16 (16H)
        // 1.0f32 big-endian is 0x3F800000; 1.0 in FP16 is 0x3C00
        let out_transcode = Command::new(BDD_BIN)
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
        let out_fp8 = Command::new(BDD_BIN)
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
    let out = Command::new(BDD_BIN)
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
    let out_chained = Command::new(BDD_BIN)
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

#[test]
fn test_infinite_generators() {
    // Pipe infinite zeros into head -c 16
    let out_zeros = Command::new("bash")
        .args([
            "-c",
            &format!("{} --input-zeros 2>/dev/null | head -c 16 | wc -c", BDD_BIN),
        ])
        .output()
        .expect("failed to run infinite zeros");
    assert!(out_zeros.status.success());
    let stdout_zeros = String::from_utf8(out_zeros.stdout).unwrap();
    assert_eq!(stdout_zeros.trim(), "16");

    // Pipe infinite random into head -c 32
    let out_rand = Command::new("bash")
        .args([
            "-c",
            &format!(
                "{} --input-random 2>/dev/null | head -c 32 | wc -c",
                BDD_BIN
            ),
        ])
        .output()
        .expect("failed to run infinite random");
    assert!(out_rand.status.success());
    let stdout_rand = String::from_utf8(out_rand.stdout).unwrap();
    assert_eq!(stdout_rand.trim(), "32");
}

#[test]
fn test_named_patterns_and_presets() {
    // Test preset mp3-header on sample.mp3
    let out_mp3 = Command::new(BDD_BIN)
        .args([
            "--preset=mp3-header",
            "--input-file=contrib/data/sample.mp3",
            "--count=1",
        ])
        .output()
        .expect("failed to run bdd preset mp3-header");
    assert!(out_mp3.status.success());
    let stdout_mp3 = String::from_utf8(out_mp3.stdout).unwrap();
    assert!(stdout_mp3.contains("\"sync\":2047"));
    assert!(stdout_mp3.contains("\"bitrate\":9"));
    assert!(stdout_mp3.contains("\"layer\":2"));

    // Test preset nvfp4 on sample_nvfp4.bin
    #[cfg(feature = "small-floats")]
    {
        let out_nvfp4 = Command::new(BDD_BIN)
            .args([
                "--preset=nvfp4",
                "--input-file=contrib/data/sample_nvfp4.bin",
                "--count=2",
            ])
            .output()
            .expect("failed to run bdd preset nvfp4");
        assert!(out_nvfp4.status.success());
        let stdout_nvfp4 = String::from_utf8(out_nvfp4.stdout).unwrap();
        let lines: Vec<&str> = stdout_nvfp4.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"w0\":0.0") && lines[0].contains("\"w1\":0.5"));
        assert!(lines[1].contains("\"w0\":1.0") && lines[1].contains("\"w1\":1.5"));
    }

    // Test preset ipv4-header on sample_ipv4.bin
    let out_ipv4 = Command::new(BDD_BIN)
        .args([
            "--preset=ipv4-header",
            "--input-file=contrib/data/sample_ipv4.bin",
            "--count=1",
        ])
        .output()
        .expect("failed to run bdd preset ipv4-header");
    assert!(out_ipv4.status.success());
    let stdout_ipv4 = String::from_utf8(out_ipv4.stdout).unwrap();
    assert!(stdout_ipv4.contains("\"version\":4"));
    assert!(stdout_ipv4.contains("\"total_length\":32"));
    assert!(stdout_ipv4.contains("\"protocol\":17"));
    assert!(stdout_ipv4.contains("\"src_ip\":3232235876"));

    // Test preset udp-header on sample_udp.bin
    let out_udp = Command::new(BDD_BIN)
        .args([
            "--preset=udp-header",
            "--input-file=contrib/data/sample_udp.bin",
            "--count=1",
        ])
        .output()
        .expect("failed to run bdd preset udp-header");
    assert!(out_udp.status.success());
    let stdout_udp = String::from_utf8(out_udp.stdout).unwrap();
    assert!(stdout_udp.contains("\"src_port\":5353"));
    assert!(stdout_udp.contains("\"dst_port\":53"));
    assert!(stdout_udp.contains("\"length\":12"));

    // Test preset tcp-header on sample_tcp.bin
    let out_tcp = Command::new(BDD_BIN)
        .args([
            "--preset=tcp-header",
            "--input-file=contrib/data/sample_tcp.bin",
            "--count=1",
        ])
        .output()
        .expect("failed to run bdd preset tcp-header");
    assert!(out_tcp.status.success());
    let stdout_tcp = String::from_utf8(out_tcp.stdout).unwrap();
    assert!(stdout_tcp.contains("\"src_port\":51820"));
    assert!(stdout_tcp.contains("\"dst_port\":443"));
    assert!(stdout_tcp.contains("\"syn\":1"));
    assert!(stdout_tcp.contains("\"ack\":0"));

    // Test kernel presets
    let out_auxv = Command::new(BDD_BIN)
        .args(["--explain-pattern=proc-auxv"])
        .output()
        .expect("failed to run explain-pattern proc-auxv");
    assert!(out_auxv.status.success());
    let stdout_auxv = String::from_utf8(out_auxv.stdout).unwrap();
    assert!(stdout_auxv.contains("128 bits"));
    assert!(stdout_auxv.contains("type") && stdout_auxv.contains("val"));

    let out_pagemap = Command::new(BDD_BIN)
        .args(["--explain-pattern=proc-pagemap"])
        .output()
        .expect("failed to run explain-pattern proc-pagemap");
    assert!(out_pagemap.status.success());
    let stdout_pagemap = String::from_utf8(out_pagemap.stdout).unwrap();
    assert!(stdout_pagemap.contains("64 bits"));
    assert!(stdout_pagemap.contains("present") && stdout_pagemap.contains("pfn"));

    let out_pci = Command::new(BDD_BIN)
        .args(["--explain-pattern=pci-config"])
        .output()
        .expect("failed to run explain-pattern pci-config");
    assert!(out_pci.status.success());
    let stdout_pci = String::from_utf8(out_pci.stdout).unwrap();
    assert!(stdout_pci.contains("vendor_id") && stdout_pci.contains("device_id"));

    // Test custom pattern with names and --json-fields
    let out_custom = Command::new(BDD_BIN)
        .args([
            "--input-zeros",
            "--count=1",
            "--input-pattern=4u4u",
            "--json-fields=hi,lo",
            "--output-json",
        ])
        .output()
        .expect("failed to run custom json fields");
    assert!(out_custom.status.success());
    let stdout_custom = String::from_utf8(out_custom.stdout).unwrap();
    assert_eq!(stdout_custom.trim(), "{\"hi\":0,\"lo\":0}");
}

#[test]
fn test_explain_pattern() {
    // Text output
    let out_text = Command::new(BDD_BIN)
        .args(["--explain-pattern=sync:11u,ver:2u,layer:2u"])
        .output()
        .expect("failed to run explain-pattern text");
    assert!(out_text.status.success());
    let stdout_text = String::from_utf8(out_text.stdout).unwrap();
    assert!(stdout_text.contains("Total Width: 15 bits"));
    assert!(stdout_text.contains("sync"));
    assert!(stdout_text.contains("ver"));
    assert!(stdout_text.contains("layer"));

    // JSON output
    let out_json = Command::new(BDD_BIN)
        .args(["--explain-pattern=sync:11u,ver:2u", "--output-json"])
        .output()
        .expect("failed to run explain-pattern json");
    assert!(out_json.status.success());
    let stdout_json = String::from_utf8(out_json.stdout).unwrap();
    assert!(stdout_json.contains("\"field_count\": 2"));
    assert!(stdout_json.contains("\"total_bits\": 13"));

    // Direct preset name lookup in explain-pattern
    let out_preset = Command::new(BDD_BIN)
        .args(["--explain-pattern=mp3-header"])
        .output()
        .expect("failed to run explain-pattern with preset");
    assert!(out_preset.status.success());
    let stdout_preset = String::from_utf8(out_preset.stdout).unwrap();
    assert!(stdout_preset.contains("32 bits"));
    assert!(stdout_preset.contains("bitrate"));
}

#[test]
fn test_download_presets_cli() {
    use tempfile::NamedTempFile;

    let sample_json = r#"[
        {
            "name": "mock-protocol",
            "description": "Mock testing protocol header",
            "pattern": "magic:8u,length:16u",
            "unit_bits": 24,
            "little_endian": false,
            "default_count": 1
        }
    ]"#;

    let src_file = NamedTempFile::new().unwrap();
    std::fs::write(src_file.path(), sample_json).unwrap();

    let dest_file = NamedTempFile::new().unwrap();

    // 1. Download presets from local file URI into custom presets-file
    let out = Command::new(BDD_BIN)
        .arg(format!(
            "--download-presets={}",
            src_file.path().to_str().unwrap()
        ))
        .arg(format!(
            "--presets-file={}",
            dest_file.path().to_str().unwrap()
        ))
        .output()
        .expect("failed to run bdd --download-presets");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Successfully downloaded and installed 1 presets"));

    // 2. Query preset from the downloaded file
    let out_list = Command::new(BDD_BIN)
        .arg(format!(
            "--presets-file={}",
            dest_file.path().to_str().unwrap()
        ))
        .arg("--list-presets")
        .output()
        .expect("failed to run bdd --list-presets");
    assert!(out_list.status.success());
    let stdout_list = String::from_utf8(out_list.stdout).unwrap();
    assert!(stdout_list.contains("mock-protocol"));
    assert!(stdout_list.contains("magic:8u,length:16u"));

    // 3. Slice using the custom preset from file
    let out_slice = Command::new(BDD_BIN)
        .arg(format!(
            "--presets-file={}",
            dest_file.path().to_str().unwrap()
        ))
        .args([
            "--input-counter",
            "--count=1",
            "--preset=mock-protocol",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd with mock-protocol");
    assert!(out_slice.status.success());
}

#[test]
fn test_presets_discovery_and_environment_variable() {
    use tempfile::NamedTempFile;

    let sample_json = r#"[
        {
            "name": "env-proto",
            "description": "Environment variable protocol",
            "pattern": "tag:4u,payload:12u",
            "unit_bits": 16,
            "little_endian": false,
            "default_count": 1
        }
    ]"#;

    let env_file = NamedTempFile::new().unwrap();
    std::fs::write(env_file.path(), sample_json).unwrap();

    let out = Command::new(BDD_BIN)
        .env("BDD_PRESETS_FILE", env_file.path().to_str().unwrap())
        .arg("--list-presets")
        .output()
        .expect("failed to run bdd --list-presets with BDD_PRESETS_FILE");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("env-proto"));
    assert!(stdout.contains("tag:4u,payload:12u"));
}

#[test]
fn test_probe_binary() {
    let out_probe = Command::new(BDD_BIN)
        .args(["--probe=contrib/data/sample.mp3", "--output-json"])
        .output()
        .expect("failed to run probe json");
    assert!(out_probe.status.success());
    let stdout_probe = String::from_utf8(out_probe.stdout).unwrap();
    assert!(stdout_probe.contains("\"sample_bytes\": 939"));
    assert!(stdout_probe.contains("\"entropy\":"));
}

#[test]
fn test_mcp_server_protocol() {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new(BDD_BIN)
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd --mcp");

    {
        let stdin = child.stdin.as_mut().expect("failed to open child stdin");
        // 1. initialize
        writeln!(stdin, "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{{\"protocolVersion\":\"2024-11-05\"}}}}").unwrap();
        // 2. tools/list
        writeln!(
            stdin,
            "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\"}}"
        )
        .unwrap();
        // 3. tools/call bdd_list_presets
        writeln!(stdin, "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"tools/call\",\"params\":{{\"name\":\"bdd_list_presets\",\"arguments\":{{}}}}}}").unwrap();
        // 4. tools/call bdd_slice on sample.mp3
        writeln!(stdin, "{{\"jsonrpc\":\"2.0\",\"id\":4,\"method\":\"tools/call\",\"params\":{{\"name\":\"bdd_slice\",\"arguments\":{{\"file_path\":\"contrib/data/sample.mp3\",\"preset\":\"mp3-header\",\"count\":1}}}}}}").unwrap();
        // 5. tools/call bdd_probe_units on sample.mp3
        writeln!(stdin, "{{\"jsonrpc\":\"2.0\",\"id\":5,\"method\":\"tools/call\",\"params\":{{\"name\":\"bdd_probe_units\",\"arguments\":{{\"file_path\":\"contrib/data/sample.mp3\",\"count\":100}}}}}}").unwrap();
    }

    let output = child
        .wait_with_output()
        .expect("failed to wait for bdd --mcp");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 5);

    // Line 1: initialize
    assert!(lines[0].contains(&format!(
        "\"serverInfo\":{{\"name\":\"bdd-mcp\",\"version\":\"{}\"}}",
        env!("CARGO_PKG_VERSION")
    )));
    // Line 2: tools/list
    assert!(lines[1].contains("\"name\":\"bdd_slice\""));
    assert!(lines[1].contains("\"name\":\"bdd_probe\""));
    assert!(lines[1].contains("\"name\":\"bdd_probe_units\""));
    // Line 3: bdd_list_presets
    assert!(lines[2].contains("mp3-header"));
    #[cfg(feature = "small-floats")]
    assert!(lines[2].contains("nvfp4"));
    // Line 4: bdd_slice output
    assert!(lines[3].contains("sync") && lines[3].contains("2047"));
    assert!(lines[3].contains("bitrate") && lines[3].contains("9"));
    // Line 5: bdd_probe_units output
    assert!(lines[4].contains("total_units") && lines[4].contains("unit_entropy"));
}

#[test]
fn test_periodic_gap_seeking() {
    // Create sparse file with 3 bytes placed at 1MB intervals
    let path = "/tmp/bdd_test_periodic_gap_seek.bin";
    {
        use std::io::Seek;
        let mut f = File::create(path).unwrap();
        f.write_all(&[0x11]).unwrap();
        f.seek(std::io::SeekFrom::Start(1024 * 1024)).unwrap();
        f.write_all(&[0x22]).unwrap();
        f.seek(std::io::SeekFrom::Start(2 * 1024 * 1024)).unwrap();
        f.write_all(&[0x33]).unwrap();
    }

    // Using raw unit of 1MB (1024*1024*8 bits) with 8-bit active unit automatically computes
    // periodic container gap of 1MB - 1 byte, triggering fast seeking between records.
    let output = Command::new(BDD_BIN)
        .args([
            "--input-file",
            path,
            "--input-raw-unit=1024*1024*8",
            "--input-unit=8",
            "--count=3",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd gap seek");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let tokens: Vec<&str> = stdout.split_whitespace().collect();
    assert_eq!(tokens, vec!["11", "22", "33"]);

    let _ = std::fs::remove_file(path);
}

#[test]
fn test_output_unit_default_to_8_bits() {
    // When --output-unit and --output-pattern are omitted, output unit defaults to 8 bits
    let output = Command::new(BDD_BIN)
        .args([
            "--input-counter",
            "--count=4",
            "--input-unit=3",
            "--output-bits",
        ])
        .output()
        .expect("failed to run bdd counter 3-bit");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let tokens: Vec<&str> = stdout.split_whitespace().collect();
    // 3-bit values zero-padded to 8-bit output units: 0 -> 00000000, 1 -> 00000001, 2 -> 00000010, 3 -> 00000011
    assert_eq!(tokens, vec!["00000000", "00000001", "00000010", "00000011"]);
}

#[test]
fn test_output_pattern_field_discard() {
    // Test discarding field in output pattern via 'x'
    let mut child = Command::new(BDD_BIN)
        .args(["--input-tuples", "--output-pattern=8U,x,8U", "--output-hex"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "10,20,30").unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait for bdd");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    // 10 -> 0x0A, 20 is dropped by x, 30 -> 0x1E => 16-bit unit 0x0A1E
    assert_eq!(stdout.trim(), "0a1e");
}

#[test]
fn test_bare_pattern_types() {
    // Bare type letters without preceding counts: x, u, b, B, f, d
    let output = Command::new(BDD_BIN)
        .args(["--explain-pattern=x,u,b,B,f,d", "--output-json"])
        .output()
        .expect("failed to run bdd explain bare");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let val: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(val["total_bits"], 1 + 1 + 1 + 8 + 32 + 64);
    assert_eq!(val["fields"][0]["bits"], 1); // x
    assert_eq!(val["fields"][1]["bits"], 1); // u
    assert_eq!(val["fields"][2]["bits"], 1); // b
    assert_eq!(val["fields"][3]["bits"], 8); // B
    assert_eq!(val["fields"][4]["bits"], 32); // f
    assert_eq!(val["fields"][5]["bits"], 64); // d
}

#[test]
fn test_embedded_counter_pattern() {
    // Packing with embedded counter (8K): counter doesn't consume from tuple
    let mut child = Command::new(BDD_BIN)
        .args(["--input-tuples", "--output-pattern=8K,8U", "--output-hex"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "10").unwrap();
        writeln!(stdin, "20").unwrap();
        writeln!(stdin, "30").unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait for bdd");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let tokens: Vec<&str> = stdout.split_whitespace().collect();
    // Record 0: counter 0, val 10 (0x000A)
    // Record 1: counter 1, val 20 (0x0114)
    // Record 2: counter 2, val 30 (0x021E)
    assert_eq!(tokens, vec!["000a", "0114", "021e"]);
}

#[test]
fn test_quoted_strings_in_input_tuples() {
    // Distinguish quoted numbers as string bytes from raw unquoted numbers
    let mut child = Command::new(BDD_BIN)
        .args([
            "--input-tuples",
            "--output-json",
            "--json-object",
            "--json-fields=s1,n1,s2,n2",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "\"123\",456,\"hello, world\",789").unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait for bdd");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let val: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();

    assert_eq!(val["s1"], "123");
    assert_eq!(val["n1"], 456);
    assert_eq!(val["s2"], "hello, world");
    assert_eq!(val["n2"], 789);

    // Verify raw ASCII byte output when packed into hex
    let mut child2 = Command::new(BDD_BIN)
        .args(["--input-tuples", "--output-hex"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");
    {
        let stdin = child2.stdin.as_mut().unwrap();
        write!(stdin, "\"5\"").unwrap();
    }
    let output2 = child2.wait_with_output().expect("failed to wait for bdd");
    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert_eq!(stdout2.trim(), "35");
}

#[test]
fn test_radix_numbers_in_input_tuples_and_integers() {
    // 1. Test hex, octal, binary, and negative literals in --input-tuples
    let mut child = Command::new(BDD_BIN)
        .args(["--input-tuples", "--output-tuples"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(stdin, "0xFF, 0o77, 0b1010, 42, -0x10").unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait for bdd");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "255,63,10,42,-16");

    // 2. Test packing mixed-radix tuples to binary output
    let mut child2 = Command::new(BDD_BIN)
        .args(["--input-tuples", "--output-pattern=8U8U8U", "--output-hex"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let stdin = child2.stdin.as_mut().unwrap();
        writeln!(stdin, "0x12, 0o77, 0b10110011").unwrap();
    }

    let output2 = child2.wait_with_output().expect("failed to wait for bdd");
    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    assert_eq!(stdout2.trim(), "123fb3");

    // 3. Test radix numbers in --input-integers
    let mut child3 = Command::new(BDD_BIN)
        .args(["--input-integers", "--output-hex"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let stdin = child3.stdin.as_mut().unwrap();
        writeln!(stdin, "0xFF\n0o77\n0b1010\n42").unwrap();
    }

    let output3 = child3.wait_with_output().expect("failed to wait for bdd");
    assert!(output3.status.success());
    let stdout3 = String::from_utf8(output3.stdout).unwrap();
    assert_eq!(stdout3.trim(), "ff 3f 0a 2a");
}

#[test]
fn test_input_repeat_modes() {
    // 1. Test stdin repeat with --input-repeat
    let mut child = Command::new(BDD_BIN)
        .args(["--input-repeat=3"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let stdin = child.stdin.as_mut().unwrap();
        write!(stdin, "AB").unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait for bdd");
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "ABABAB");

    // 2. Test alias --repeat-input
    let mut child2 = Command::new(BDD_BIN)
        .args(["--repeat-input=3"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let stdin = child2.stdin.as_mut().unwrap();
        write!(stdin, "XY").unwrap();
    }

    let output2 = child2.wait_with_output().expect("failed to wait for bdd");
    assert!(output2.status.success());
    assert_eq!(String::from_utf8(output2.stdout).unwrap(), "XYXYXY");

    // 3. Test --input-tuples with --input-repeat
    let mut child3 = Command::new(BDD_BIN)
        .args(["--input-tuples", "--input-repeat=2", "--output-tuples"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let stdin = child3.stdin.as_mut().unwrap();
        writeln!(stdin, "1,2\n3,4").unwrap();
    }

    let output3 = child3.wait_with_output().expect("failed to wait for bdd");
    assert!(output3.status.success());
    let stdout3 = String::from_utf8(output3.stdout).unwrap();
    assert_eq!(stdout3.trim(), "1,2\n3,4\n1,2\n3,4");

    // 4. Test regular file with --input-repeat=0 and --count=5 (infinite repeat bounded by count)
    let tmp_file = "/tmp/bdd_repeat_test.bin";
    std::fs::write(tmp_file, [0x10, 0x20]).unwrap();
    let output4 = Command::new(BDD_BIN)
        .args([
            &format!("--input-file={}", tmp_file),
            "--input-repeat=0",
            "--count=5",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd");
    let _ = std::fs::remove_file(tmp_file);

    assert!(output4.status.success());
    let stdout4 = String::from_utf8(output4.stdout).unwrap();
    assert_eq!(stdout4.trim(), "10 20 10 20 10");
}

#[test]
fn test_multi_file_merge_round_robin() {
    let m1_path = "/tmp/bdd_merge_test_m1.bin";
    let m2_path = "/tmp/bdd_merge_test_m2.bin";
    std::fs::write(m1_path, [0x11, 0x22]).unwrap();
    std::fs::write(m2_path, [0xAA, 0xBB]).unwrap();

    // Repeatable --merge-file
    let output = Command::new(BDD_BIN)
        .args([
            "--input-counter",
            "--count=2",
            "--input-unit=8",
            "--output-unit=8",
            "--merge-file",
            m1_path,
            "--merge-file",
            m2_path,
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd multi merge");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let tokens: Vec<&str> = stdout.split_whitespace().collect();
    // Round robin: Primary 00, m1 11, m2 aa, Primary 01, m1 22, m2 bb
    assert_eq!(tokens, vec!["00", "11", "aa", "01", "22", "bb"]);

    // Comma-separated --merge-files
    let merge_files_arg = format!("{},{}", m1_path, m2_path);
    let output2 = Command::new(BDD_BIN)
        .args([
            "--input-counter",
            "--count=2",
            "--input-unit=8",
            "--output-unit=8",
            "--merge-files",
            &merge_files_arg,
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd multi merge with --merge-files");

    assert!(output2.status.success());
    let stdout2 = String::from_utf8(output2.stdout).unwrap();
    let tokens2: Vec<&str> = stdout2.split_whitespace().collect();
    assert_eq!(tokens2, vec!["00", "11", "aa", "01", "22", "bb"]);

    let _ = std::fs::remove_file(m1_path);
    let _ = std::fs::remove_file(m2_path);
}

#[test]
fn test_drop_partial_eof_cli() {
    let p = "/tmp/bdd_test_drop_eof.bin";
    // 1 byte = 8 bits (0000 0001)
    std::fs::write(p, [0x01]).unwrap();

    // Default: unit size 3 pads remaining 2 bits (010 = 2) => 3 units: 00 00 02 (default 8-bit output unit)
    let out_default = Command::new(BDD_BIN)
        .args(["--input-file", p, "--input-unit=3", "--output-hex"])
        .output()
        .expect("failed to run bdd pad eof");
    assert!(out_default.status.success());
    let stdout_def = String::from_utf8(out_default.stdout).unwrap();
    let tokens_def: Vec<&str> = stdout_def.split_whitespace().collect();
    assert_eq!(tokens_def, vec!["00", "00", "02"]);

    // With --drop-partial-eof: drops incomplete trailing 2 bits => 2 units: 00 00
    let out_drop = Command::new(BDD_BIN)
        .args([
            "--input-file",
            p,
            "--input-unit=3",
            "--drop-partial-eof",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd drop eof");
    assert!(out_drop.status.success());
    let stdout_drop = String::from_utf8(out_drop.stdout).unwrap();
    let tokens_drop: Vec<&str> = stdout_drop.split_whitespace().collect();
    assert_eq!(tokens_drop, vec!["00", "00"]);

    // With alias --drop-trailing-bits
    let out_alias = Command::new(BDD_BIN)
        .args([
            "--input-file",
            p,
            "--input-unit=3",
            "--drop-trailing-bits",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd drop alias");
    assert!(out_alias.status.success());
    let stdout_alias = String::from_utf8(out_alias.stdout).unwrap();
    let tokens_alias: Vec<&str> = stdout_alias.split_whitespace().collect();
    assert_eq!(tokens_alias, vec!["00", "00"]);

    let _ = std::fs::remove_file(p);
}

#[test]
fn test_terminal_hex_and_bits_no_trailing_whitespace() {
    // Hex output
    let out_hex = Command::new(BDD_BIN)
        .args([
            "--input-counter",
            "--count=16",
            "--input-unit=8",
            "--output-hex",
        ])
        .output()
        .expect("failed to run hex output");
    assert!(out_hex.status.success());
    let stdout_hex = String::from_utf8(out_hex.stdout).unwrap();
    assert!(!stdout_hex.is_empty());
    for line in stdout_hex.lines() {
        assert!(
            !line.ends_with(' '),
            "Hex output line has trailing space: {:?}",
            line
        );
    }

    // Bit output
    let out_bits = Command::new(BDD_BIN)
        .args([
            "--input-counter",
            "--count=16",
            "--input-unit=4",
            "--output-bits",
        ])
        .output()
        .expect("failed to run bit output");
    assert!(out_bits.status.success());
    let stdout_bits = String::from_utf8(out_bits.stdout).unwrap();
    assert!(!stdout_bits.is_empty());
    for line in stdout_bits.lines() {
        assert!(
            !line.ends_with(' '),
            "Bit output line has trailing space: {:?}",
            line
        );
    }
}

#[test]
fn test_web_server_decoupled_python() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::thread;
    use std::time::Duration;

    let port = 17894;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let script = format!("{}/web/server.py", manifest_dir);
    let mut child = Command::new("python3")
        .arg(&script)
        .arg(format!("{}", port))
        .env("BDD_BIN", BDD_BIN)
        .spawn()
        .expect("failed to spawn python3 web/server.py");

    // Wait for server to start
    let mut connected = false;
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(50));
        if TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            connected = true;
            break;
        }
    }
    assert!(connected, "Failed to connect to bdd web server");

    // 1. Test GET /api/status
    {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .write_all(b"GET /api/status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("\"status\""));
    }

    // 2. Test GET / (HTML index)
    {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("bdd"));
        assert!(resp.contains("Interactive Bitstream Slicer"));
    }

    // 3. Test POST /api/explain
    {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let body = r#"{"pattern":"sync:11u,ver:2u"}"#;
        let req = format!(
            "POST /api/explain HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("sync"));
        assert!(resp.contains("ver"));
    }

    // 4. Test POST /api/process
    {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let body = r#"{"args":["--input-zeros","--count=2","--output-hex"]}"#;
        let req = format!(
            "POST /api/process HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("00 00"));
    }

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_web_server_decoupled_rust() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::thread;
    use std::time::Duration;

    let port = 17893;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let rust_server_bin = format!("{}/web/target/debug/bdd-web", manifest_dir);
    if !std::path::Path::new(&rust_server_bin).exists() {
        return;
    }
    let mut child = Command::new(&rust_server_bin)
        .arg(format!("{}", port))
        .env("BDD_BIN", BDD_BIN)
        .spawn()
        .expect("failed to spawn bdd-web");

    let mut connected = false;
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(50));
        if TcpStream::connect(format!("127.0.0.1:{}", port)).is_ok() {
            connected = true;
            break;
        }
    }
    assert!(connected, "Failed to connect to bdd-web standalone server");

    // 1. Test GET /api/status
    {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        stream
            .write_all(b"GET /api/status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("\"backend\":\"standalone-rust\""));
    }

    // 2. Test POST /api/explain
    {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let body = r#"{"pattern":"sync:11u,ver:2u"}"#;
        let req = format!(
            "POST /api/explain HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("sync"));
        assert!(resp.contains("ver"));
    }

    // 3. Test POST /api/process
    {
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap();
        let body = r#"{"args":["--input-zeros","--count=2","--output-hex"]}"#;
        let req = format!(
            "POST /api/process HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        assert!(resp.contains("200 OK"));
        assert!(resp.contains("00 00"));
    }

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_warning_deduplication_and_summary() {
    let output = Command::new(BDD_BIN)
        .args([
            "--input-counter",
            "--count=10",
            "--input-unit=8",
            "--rearrange=10",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd with missing rearrange field");

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "00 00 00 00 00 00 00 00 00 00");

    // The initial warning was printed
    assert!(stderr.contains("Field 10 mentioned in --rearrange missing"));
    // The EOF summary was printed
    assert!(stderr
        .contains("[bdd] Warning: 'Field 10 mentioned in --rearrange missing' repeated 10 times"));
    assert!(stderr.contains("[bdd] Warning: 'No fields to output, assumed 0' repeated 10 times"));

    // Ensure it wasn't printed 10 times individually (1 initial + 1 in summary quote = 2)
    let rearrange_mentions = stderr
        .matches("Field 10 mentioned in --rearrange missing")
        .count();
    assert_eq!(rearrange_mentions, 2);
}

#[test]
fn test_quiet_flag_suppresses_warnings_and_summary() {
    for quiet_flag in ["-q", "--quiet"] {
        let output = Command::new(BDD_BIN)
            .args([
                "--input-counter",
                "--count=10",
                "--input-unit=8",
                "--rearrange=10",
                "--output-hex",
                quiet_flag,
            ])
            .output()
            .expect("failed to run bdd with quiet flag");

        assert!(output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.is_empty(),
            "stderr should be completely empty with {}, got: {}",
            quiet_flag,
            stderr
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert_eq!(stdout.trim(), "00 00 00 00 00 00 00 00 00 00");
    }
}

#[test]
fn test_stream_warning_deduplication() {
    let mut child = Command::new(BDD_BIN)
        .args([
            "--input-integers",
            "--count=5",
            "--input-unit=8",
            "--output-hex",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"bad\nbad\nbad\nbad\nbad\n").unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Non-integer 'bad' interpreted as zero"));
    assert!(
        stderr.contains("[bdd] Warning: 'Non-integer 'bad' interpreted as zero' repeated 5 times")
    );
}

#[test]
fn test_pattern_with_raw_unit_and_unit_overwrite() {
    // 1. Pattern with raw-unit: extract 8-bit pattern from 16-bit container
    let out_raw = Command::new(BDD_BIN)
        .args([
            "--input-zeros",
            "--count=2",
            "--input-raw-unit=16",
            "--input-pattern=8U",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd with pattern and raw unit");
    assert!(out_raw.status.success());
    let stdout_raw = String::from_utf8(out_raw.stdout).unwrap();
    assert_eq!(stdout_raw.trim(), "00 00");

    // 2. Pattern with unit: emits warning and pattern overwrites unit
    let out_overwrite = Command::new(BDD_BIN)
        .args([
            "--input-zeros",
            "--count=2",
            "--input-pattern=8U",
            "--input-unit=8",
            "--output-hex",
        ])
        .output()
        .expect("failed to run bdd with pattern and unit");
    assert!(out_overwrite.status.success());
    let stderr_overwrite = String::from_utf8(out_overwrite.stderr).unwrap();
    assert!(stderr_overwrite.contains("--input-pattern overwrites --input-unit"));
    let stdout_overwrite = String::from_utf8(out_overwrite.stdout).unwrap();
    assert_eq!(stdout_overwrite.trim(), "00 00");
}

#[test]
fn test_web_server_decoupled_notice() {
    let out = Command::new(BDD_BIN)
        .arg("--serve")
        .output()
        .expect("failed to run bdd --serve");
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("decoupled to the 'web/' directory"));
}

#[cfg(not(feature = "small-floats"))]
#[test]
fn test_small_floats_disabled_error() {
    let out_pat = Command::new(BDD_BIN)
        .args(["--input-pattern=16H", "--output-hex"])
        .output()
        .expect("failed to run bdd with 16H");
    assert!(!out_pat.status.success());
    let stderr_pat = String::from_utf8(out_pat.stderr).unwrap();
    assert!(stderr_pat.contains("small-floats"));

    let out_preset = Command::new(BDD_BIN)
        .args(["--preset=nvfp4"])
        .output()
        .expect("failed to run bdd with nvfp4");
    assert!(!out_preset.status.success());
    let stderr_preset = String::from_utf8(out_preset.stderr).unwrap();
    assert!(stderr_preset.contains("small-floats"));
}

#[test]
fn test_llms_flag() {
    let out = Command::new(BDD_BIN)
        .arg("--llms")
        .output()
        .expect("failed to run bdd --llms");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("# Bit Dump & Dissect (bdd) - LLM & Agent Reference"));
    assert!(stdout.contains("--mcp"));

    let out_alias = Command::new(BDD_BIN)
        .arg("--ai-guide")
        .output()
        .expect("failed to run bdd --ai-guide");
    assert!(out_alias.status.success());
    let stdout_alias = String::from_utf8(out_alias.stdout).unwrap();
    assert_eq!(stdout, stdout_alias);
}

#[test]
fn test_stream_io_pattern_simple_arrow() {
    let out = Command::new(BDD_BIN)
        .args(["8->3", "--input-counter", "--count=4", "--output-bits"])
        .output()
        .expect("failed to run bdd 8->3");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let tokens: Vec<&str> = stdout.split_whitespace().collect();
    assert_eq!(tokens, vec!["000", "001", "010", "011"]);
}

#[test]
fn test_stream_io_pattern_container_forms() {
    // 123 bits skip: 123 zeros
    // Container 1: 2 bits (00) + 4 bits (1011 = 11) + 2 bits (00)
    // Gap: 8 bits (11111111)
    // Container 2: 2 bits (00) + 4 bits (1100 = 12) + 2 bits (00)
    let mut bits = String::new();
    for _ in 0..123 {
        bits.push('0');
    }
    bits.push_str("00101100"); // Container 1: 0x2C
    bits.push_str("11111111"); // Gap: 8 bits
    bits.push_str("00110000"); // Container 2: 0x30
    while !bits.len().is_multiple_of(8) {
        bits.push('0');
    }

    let mut bytes = Vec::new();
    for chunk in bits.as_bytes().chunks(8) {
        let s = std::str::from_utf8(chunk).unwrap();
        bytes.push(u8::from_str_radix(s, 2).unwrap());
    }

    let p_in = "/tmp/bdd_test_stream_pattern_in.bin";
    std::fs::write(p_in, &bytes).unwrap();

    // Test Form A: 123:8[2:4]+8 -> 5B:8[2:4]
    let out_a = Command::new(BDD_BIN)
        .args([
            "123:8[2:4]+8 -> 5B:8[2:4]",
            "--input-file",
            p_in,
            "--count=2",
        ])
        .output()
        .expect("failed to run Form A stream pattern");
    assert!(out_a.status.success());
    assert_eq!(out_a.stdout, vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x30]);

    // Test Form B: 123:[2:4:2]+8 -> 5B:[2:4:2]
    let out_b = Command::new(BDD_BIN)
        .args([
            "123:[2:4:2]+8 -> 5B:[2:4:2]",
            "--input-file",
            p_in,
            "--count=2",
        ])
        .output()
        .expect("failed to run Form B stream pattern");
    assert!(out_b.status.success());
    assert_eq!(out_b.stdout, vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x30]);

    // Test explicit output framing options mirror:
    let out_opts = Command::new(BDD_BIN)
        .args([
            "--input-skip-bits=123",
            "--input-raw-unit=8",
            "--input-offset=2",
            "--input-unit=4",
            "--input-gap=8",
            "--output-skip-bits=40",
            "--output-raw-unit=8",
            "--output-offset=2",
            "--output-unit=4",
            "--output-gap=0",
            "--input-file",
            p_in,
            "--count=2",
        ])
        .output()
        .expect("failed to run explicit output options");
    assert!(out_opts.status.success());
    assert_eq!(
        out_opts.stdout,
        vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x2C, 0x30]
    );

    let _ = std::fs::remove_file(p_in);
}

#[test]
fn test_positional_tuple_patterns() {
    // 1. Arrow pipeline syntax: unpack two 4-bit nibbles from each byte, swap fields, repack:
    let mut child_bin = Command::new(BDD_BIN)
        .args(["4U4U -> 4U4U", "--rearrange=1,0", "--output-hex"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        use std::io::Write;
        let stdin = child_bin.stdin.as_mut().unwrap();
        stdin.write_all(&[0x12, 0x34]).unwrap();
    }

    let out_bin = child_bin
        .wait_with_output()
        .expect("failed to wait for bdd");
    assert!(out_bin.status.success());
    let stdout_bin = String::from_utf8(out_bin.stdout).unwrap();
    let tokens_bin: Vec<&str> = stdout_bin.split_whitespace().collect();
    assert_eq!(tokens_bin, vec!["21", "43"]);

    // 2. Explicit -p and -P flags:
    let mut child_flags = Command::new(BDD_BIN)
        .args([
            "-p",
            "4U4U",
            "-P",
            "4U4U",
            "--rearrange=1,0",
            "--output-hex",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        use std::io::Write;
        let stdin = child_flags.stdin.as_mut().unwrap();
        stdin.write_all(&[0x12, 0x34]).unwrap();
    }

    let out_flags = child_flags
        .wait_with_output()
        .expect("failed to wait for bdd");
    assert!(out_flags.status.success());
    let stdout_flags = String::from_utf8(out_flags.stdout).unwrap();
    let tokens_flags: Vec<&str> = stdout_flags.split_whitespace().collect();
    assert_eq!(tokens_flags, vec!["21", "43"]);

    // 3. Single positional input pattern: unpack binary into tuples
    let mut child_in = Command::new(BDD_BIN)
        .args(["4U4U", "--output-tuples"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        use std::io::Write;
        let stdin = child_in.stdin.as_mut().unwrap();
        stdin.write_all(&[0x12, 0x34]).unwrap();
    }

    let out_in = child_in.wait_with_output().expect("failed to wait for bdd");
    assert!(out_in.status.success());
    let stdout_in = String::from_utf8(out_in.stdout).unwrap();
    let lines_in: Vec<&str> = stdout_in.trim().split('\n').collect();
    assert_eq!(lines_in, vec!["1,2", "3,4"]);

    // 4. Text tuples with single positional output pattern: pack comma-separated pairs ("1,2") into two 4-bit nibbles per byte (0x12):
    let mut child_txt = Command::new(BDD_BIN)
        .args(["4U4U", "--input-tuples", "--output-hex"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        use std::io::Write;
        let stdin = child_txt.stdin.as_mut().unwrap();
        writeln!(stdin, "1,2").unwrap();
        writeln!(stdin, "3,4").unwrap();
    }

    let out_txt = child_txt
        .wait_with_output()
        .expect("failed to wait for bdd");
    assert!(out_txt.status.success());
    let stdout_txt = String::from_utf8(out_txt.stdout).unwrap();
    let tokens_txt: Vec<&str> = stdout_txt.split_whitespace().collect();
    assert_eq!(tokens_txt, vec!["12", "34"]);

    // 5. Multiple positional parameters are rejected:
    let out_rejected = Command::new(BDD_BIN)
        .args(["4U4U", "4U4U"])
        .output()
        .expect("failed to run bdd");
    assert!(!out_rejected.status.success());
}

#[test]
fn test_nested_pattern_parentheses() {
    // 1. Check CLI explain-pattern with 2*(2*(u2*U))
    let output = Command::new(BDD_BIN)
        .args(["--explain-pattern=2*(2*(u2*U))"])
        .output()
        .expect("failed to run bdd explain");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Total Width: 12 bits"));
    assert!(stdout.contains("Fields:      12"));

    // 2. Unpack binary data using 2*(2*(u2*U)) into 12-field CSV tuples
    let mut child = Command::new(BDD_BIN)
        .args(["2*(2*(u2*U))", "--output-tuples"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        // 2 bytes = 16 bits; first 12 bits will form one tuple: 0b10110011, 0b11110000
        stdin.write_all(&[0xB3, 0xF0]).unwrap();
    }

    let out = child.wait_with_output().expect("failed to wait for bdd");
    assert!(out.status.success());
    let tuple_line = String::from_utf8(out.stdout).unwrap();
    let first_line = tuple_line.lines().next().unwrap();
    let fields: Vec<&str> = first_line.split(',').collect();
    assert_eq!(fields.len(), 12);

    // 3. Check 3-level arbitrary nesting: 2*(2*(2*(u2*U))) -> 24 bits, 24 fields
    let output3 = Command::new(BDD_BIN)
        .args(["--explain-pattern=2*(2*(2*(u2*U)))"])
        .output()
        .expect("failed to run bdd explain");
    assert!(output3.status.success());
    let stdout3 = String::from_utf8(output3.stdout).unwrap();
    assert!(stdout3.contains("Total Width: 24 bits"));
    assert!(stdout3.contains("Fields:      24"));

    // 4. Check nested parentheses with commas: 2*(4U, 4u) -> 16 bits, 4 fields
    let output_commas = Command::new(BDD_BIN)
        .args(["--explain-pattern=2*(4U, 4u)"])
        .output()
        .expect("failed to run bdd explain");
    assert!(output_commas.status.success());
    let stdout_commas = String::from_utf8(output_commas.stdout).unwrap();
    assert!(stdout_commas.contains("Total Width: 16 bits"));
    assert!(stdout_commas.contains("Fields:      4"));

    // 5. Bare parentheses without multiplier: (u2*U) -> 3 bits, 3 fields
    let output_bare = Command::new(BDD_BIN)
        .args(["--explain-pattern=(u2*U)"])
        .output()
        .expect("failed to run bdd explain");
    assert!(output_bare.status.success());
    let stdout_bare = String::from_utf8(output_bare.stdout).unwrap();
    assert!(stdout_bare.contains("Total Width: 3 bits"));
    assert!(stdout_bare.contains("Fields:      3"));
}

#[test]
fn test_probe_units_and_crypto_keys() {
    // 1. Test unit probe on counter stream
    let out = Command::new(BDD_BIN)
        .args(["--input-counter", "--count=256", "--probe-units"])
        .output()
        .expect("failed to run bdd --probe-units");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("bdd Unit Stream Prober"));
    assert!(stdout.contains("Sample Size:         256 units"));
    assert!(stdout.contains("Unit Width:          8 bits"));
    assert!(stdout.contains("Shannon Entropy:     8.0000 / 8.0000 bits/unit"));
    assert!(stdout.contains("Potential Maximum-Entropy Cryptographic Keys"));

    // 2. Test JSON output format
    let out_json = Command::new(BDD_BIN)
        .args([
            "--input-counter",
            "--count=64",
            "--probe-units",
            "--output-json",
        ])
        .output()
        .expect("failed to run bdd --probe-units --output-json");
    assert!(out_json.status.success());
    let stdout_json = String::from_utf8(out_json.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout_json).expect("valid json output");
    assert_eq!(v["total_units"], 64);
    assert_eq!(v["unit_bits"], 8);
    assert!(v["crypto_key_candidates"].is_array());

    // 3. Test finding embedded crypto key after input skip
    let file_path = "/tmp/bdd_embedded_key_test.bin";
    let mut file_data = vec![0u8; 64];
    let key_bytes = [
        0x60, 0x3d, 0xeb, 0x10, 0x15, 0xca, 0x71, 0xbe, 0x2b, 0x73, 0xae, 0xf0, 0x85, 0x7d, 0x77,
        0x81, 0x1f, 0x35, 0x2c, 0x07, 0x3b, 0x61, 0x08, 0xd7, 0x2d, 0x98, 0x10, 0xa3, 0x09, 0x14,
        0xdf, 0xf4,
    ];
    file_data.extend_from_slice(&key_bytes);
    file_data.extend_from_slice(&[0u8; 64]);
    std::fs::write(file_path, &file_data).unwrap();

    // Run --probe-keys on the file
    let out_key = Command::new(BDD_BIN)
        .args([
            &format!("--input-file={}", file_path),
            "--probe-keys=256",
            "--output-json",
        ])
        .output()
        .expect("failed to run bdd --probe-keys");
    assert!(out_key.status.success());
    let stdout_key = String::from_utf8(out_key.stdout).unwrap();
    let v_key: serde_json::Value = serde_json::from_str(&stdout_key).expect("valid json");
    let candidates = v_key["crypto_key_candidates"].as_array().unwrap();
    assert!(!candidates.is_empty());
    let top_candidate = &candidates[0];
    assert_eq!(top_candidate["start_unit"], 64);
    assert_eq!(top_candidate["bit_offset"], 512);
    assert_eq!(top_candidate["bit_length"], 256);
    assert_eq!(top_candidate["byte_length"], 32);
    let expected_hex: String = key_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    assert_eq!(top_candidate["hex_payload"], expected_hex);

    // 4. Test probing after input skip bits: skip 512 bits (64 bytes), key now at unit 0
    let out_skip = Command::new(BDD_BIN)
        .args([
            &format!("--input-file={}", file_path),
            "--input-skip-bits=512",
            "--probe-keys=256",
            "--output-json",
        ])
        .output()
        .expect("failed to run bdd with skip and probe-keys");
    assert!(out_skip.status.success());
    let v_skip: serde_json::Value = serde_json::from_slice(&out_skip.stdout).unwrap();
    assert_eq!(v_skip["crypto_key_candidates"][0]["start_unit"], 0);
    assert_eq!(v_skip["crypto_key_candidates"][0]["bit_offset"], 0);

    // 5. Test probe with pattern and probe-field
    let out_pat = Command::new(BDD_BIN)
        .args([
            "--input-counter",
            "--count=32",
            "--input-pattern=16U,32U",
            "--probe-field=1",
            "--probe-units",
            "--output-json",
        ])
        .output()
        .expect("failed to run bdd with probe-field");
    assert!(out_pat.status.success());
    let v_pat: serde_json::Value = serde_json::from_slice(&out_pat.stdout).unwrap();
    assert_eq!(v_pat["unit_bits"], 32);
    assert_eq!(v_pat["total_units"], 32);
}

#[test]
fn test_stream_memory_keys_demo() {
    // 1. Test shell script contrib/shell/stream_memory_keys.sh
    let out_sh = Command::new("bash")
        .args([
            "contrib/shell/stream_memory_keys.sh",
            "--source",
            "demo",
            "--units",
            "128",
            "--json",
        ])
        .env("BDD_BIN", BDD_BIN)
        .output()
        .expect("failed to run contrib/shell/stream_memory_keys.sh");

    assert!(
        out_sh.status.success(),
        "stream_memory_keys.sh failed with stderr: {}",
        String::from_utf8_lossy(&out_sh.stderr)
    );
    let v_sh: serde_json::Value =
        serde_json::from_slice(&out_sh.stdout).expect("valid json from stream_memory_keys.sh");
    assert_eq!(v_sh["unit_bits"], 256);
    assert_eq!(v_sh["total_units"], 128);
    let candidates_sh = v_sh["crypto_key_candidates"]
        .as_array()
        .expect("crypto_key_candidates array");
    assert!(!candidates_sh.is_empty());
    assert_eq!(candidates_sh[0]["confidence"], "Very High");
    assert_eq!(candidates_sh[0]["bit_length"], 256);

    // 2. Test python script contrib/python/stream_memory_keys.py
    let out_py = Command::new("python3")
        .args([
            "contrib/python/stream_memory_keys.py",
            "--source",
            "demo",
            "--units",
            "128",
            "--json",
        ])
        .env("BDD_BIN", BDD_BIN)
        .output()
        .expect("failed to run contrib/python/stream_memory_keys.py");

    assert!(
        out_py.status.success(),
        "stream_memory_keys.py failed with stderr: {}",
        String::from_utf8_lossy(&out_py.stderr)
    );
    let v_py: serde_json::Value =
        serde_json::from_slice(&out_py.stdout).expect("valid json from stream_memory_keys.py");
    assert_eq!(v_py["unit_bits"], 256);
    assert_eq!(v_py["total_units"], 128);
    let candidates_py = v_py["crypto_key_candidates"]
        .as_array()
        .expect("crypto_key_candidates array");
    assert!(!candidates_py.is_empty());
    assert_eq!(candidates_py[0]["confidence"], "Very High");
    assert_eq!(candidates_py[0]["bit_length"], 256);
}

#[test]
fn test_cli_code_generation() {
    let out_c = Command::new(BDD_BIN)
        .args(["--preset", "mpeg-ts", "--export-c"])
        .output()
        .expect("failed to run export-c");
    assert!(out_c.status.success());
    let c_str = String::from_utf8(out_c.stdout).unwrap();
    assert!(c_str.contains("typedef struct {"));
    assert!(c_str.contains("uint8_t sync"));
    assert!(c_str.contains("uint16_t pid : 13"));
    assert!(c_str.contains("} MpegTs;"));

    let out_rust = Command::new(BDD_BIN)
        .args(["--preset", "mpeg-ts", "--export-rust"])
        .output()
        .expect("failed to run export-rust");
    assert!(out_rust.status.success());
    let rust_str = String::from_utf8(out_rust.stdout).unwrap();
    assert!(rust_str.contains("pub struct MpegTs {"));
    assert!(rust_str.contains("pub sync: u8,"));
    assert!(rust_str.contains("pub pid: u16,"));
}

#[test]
fn test_cli_probe_visual_and_signatures() {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    // Write PNG magic header
    tmp.write_all(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])
        .unwrap();
    // Write 256 distinct bytes for entropy
    let mut rand_bytes = Vec::new();
    for i in 0..=255u8 {
        rand_bytes.push(i);
    }
    tmp.write_all(&rand_bytes).unwrap();
    tmp.flush().unwrap();

    let path = tmp.path().to_str().unwrap();

    let out_json = Command::new(BDD_BIN)
        .args(["--probe", path, "--output-json"])
        .output()
        .expect("failed to run probe json");
    assert!(out_json.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out_json.stdout).unwrap();
    let sigs = v["detected_signatures"].as_array().expect("signatures");
    assert!(!sigs.is_empty());
    assert_eq!(sigs[0]["name"], "PNG");
    assert!(!v["entropy_sparkline"].as_str().unwrap().is_empty());

    let out_map = Command::new(BDD_BIN)
        .args(["--probe", path, "--probe-visual"])
        .output()
        .expect("failed to run probe visual");
    assert!(out_map.status.success());
    let txt = String::from_utf8(out_map.stdout).unwrap();
    assert!(txt.contains("Visual Entropy Map"));
    assert!(txt.contains("Entropy Sparkline:"));
    assert!(txt.contains("Identified Magic Signatures:"));
    assert!(txt.contains("PNG"));
}

#[test]
fn test_cli_completions() {
    let out = Command::new(BDD_BIN)
        .args(["--completions", "bash"])
        .output()
        .expect("failed to run completions");
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.contains("_bdd()"));
    assert!(s.contains("complete -F _bdd"));
}

#[test]
fn test_inline_stream_arrows() {
    // 8 -> xor(0xFF) -> 8
    let mut child = Command::new(BDD_BIN)
        .args(["8 -> xor(0xFF) -> 8", "--output-hex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn bdd");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(&[0x00, 0x01, 0x02])
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "ff fe fd");

    // Chained: 8 -> add(10) -> mul(2) -> 8
    let mut child2 = Command::new(BDD_BIN)
        .args(["8 -> add(10) -> mul(2) -> 8", "--output-hex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn bdd");
    child2.stdin.as_mut().unwrap().write_all(&[0x01]).unwrap();
    let out2 = child2.wait_with_output().unwrap();
    assert!(out2.status.success());
    assert_eq!(String::from_utf8(out2.stdout).unwrap().trim(), "16");
}

#[test]
fn test_mcp_transcode_and_generate() {
    let mut child = Command::new(BDD_BIN)
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mcp server");

    let stdin = child.stdin.as_mut().unwrap();
    let stdout = child.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);

    // 1. Call bdd_generate
    let gen_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "tools/call",
        "params": {
            "name": "bdd_generate",
            "arguments": {
                "pattern": "8U,8U",
                "count": 2,
                "source": "counter",
                "output_format": "hex"
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&gen_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["id"], 101);
    let content = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(!content.is_empty());

    // 2. Call bdd_transcode
    let trans_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 102,
        "method": "tools/call",
        "params": {
            "name": "bdd_transcode",
            "arguments": {
                "input_hex": "0102",
                "input_pattern": "8U",
                "output_format": "hex",
                "manipulators": ["xor:0xFF"]
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&trans_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    line.clear();
    reader.read_line(&mut line).unwrap();
    let resp2: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp2["id"], 102);
    let trans_content = resp2["result"]["content"][0]["text"].as_str().unwrap();
    assert_eq!(trans_content.trim(), "fe fd");

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn test_unified_stream_pipeline_fusion_and_sources() {
    // 1. "zeros -> 8 -> xor(0xAA) -> hex" with count 2
    let out1 = Command::new(BDD_BIN)
        .args(["zeros -> 8 -> xor(0xAA) -> hex", "--count", "2"])
        .output()
        .expect("failed to run bdd zeros xor");
    assert!(out1.status.success());
    assert_eq!(String::from_utf8(out1.stdout).unwrap().trim(), "aa aa");

    // 2. Field fusion "4U4U -> {0|1} -> 8U -> hex"
    let mut child2 = Command::new(BDD_BIN)
        .args(["4U4U -> {0|1} -> 8U -> hex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");
    {
        let mut stdin = child2.stdin.take().unwrap();
        stdin.write_all(&[0xFA]).unwrap();
    }
    let out2 = child2.wait_with_output().unwrap();
    assert!(out2.status.success());
    assert_eq!(String::from_utf8(out2.stdout).unwrap().trim(), "fa");

    // 3. Nibble swap "4U4U -> {1, 0} -> 4U4U -> hex"
    let mut child3 = Command::new(BDD_BIN)
        .args(["4U4U -> {1, 0} -> 4U4U -> hex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");
    {
        let mut stdin = child3.stdin.take().unwrap();
        stdin.write_all(&[0xFA]).unwrap();
    }
    let out3 = child3.wait_with_output().unwrap();
    assert!(out3.status.success());
    assert_eq!(String::from_utf8(out3.stdout).unwrap().trim(), "af");
}

#[test]
fn test_multi_source_brackets_cli() {
    let out1 = Command::new(BDD_BIN)
        .args(["--count", "2", "[ zeros:8, ones:8 ] -> hex"])
        .output()
        .expect("failed to run bdd");
    assert!(out1.status.success());
    // 4 units round-robin: 0x00, 0xFF, 0x00, 0xFF
    let stdout1 = String::from_utf8(out1.stdout).unwrap();
    assert_eq!(stdout1.trim(), "00 ff 00 ff");

    let out2 = Command::new(BDD_BIN)
        .args(["--count", "2", "[ zeros, counter ] -> 8 -> hex"])
        .output()
        .expect("failed to run bdd");
    assert!(out2.status.success());
    // 4 units round-robin: 0x00, counter 0, 0x00, counter 1
    let stdout2 = String::from_utf8(out2.stdout).unwrap();
    assert_eq!(stdout2.trim(), "00 00 00 01");

    let out3 = Command::new(BDD_BIN)
        .args(["--count", "2", "[ zeros, counter ] -> 16 -> hex"])
        .output()
        .expect("failed to run bdd");
    assert!(out3.status.success());
    // 4 16-bit units round-robin: 0x0000, counter 0, 0x0000, counter 1
    let stdout3 = String::from_utf8(out3.stdout).unwrap();
    assert_eq!(stdout3.trim(), "0000 0000 0000 0001");

    let out4 = Command::new(BDD_BIN)
        .args(["--count", "2", "[ zeros:16, ones:16 ] -> hex"])
        .output()
        .expect("failed to run bdd");
    assert!(out4.status.success());
    let stdout4 = String::from_utf8(out4.stdout).unwrap();
    assert_eq!(stdout4.trim(), "0000 ffff 0000 ffff");
}

#[test]
fn test_pipe_interleave_cli() {
    let out = Command::new(BDD_BIN)
        .args(["--count", "2", "zeros:8 -> interleave(counter:8) -> hex"])
        .output()
        .expect("failed to run bdd");
    assert!(out.status.success());
    // 4 units round-robin: zeros:8 (00), counter:8 (00), zeros:8 (00), counter:8 (01)
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "00 00 00 01");
}

#[test]
fn test_pipe_tee_cli() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let tee_path = tmp.path().to_str().unwrap();

    let out = Command::new(BDD_BIN)
        .args([
            "--count",
            "2",
            &format!("zeros:8 -> add(5) -> tee('{}') -> add(1) -> hex", tee_path),
        ])
        .output()
        .expect("failed to run bdd");
    assert!(out.status.success());
    // Main stream output after add(1) is 0x06
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "06 06");

    // Tee file recorded intermediate units after add(5) = 0x05
    let tee_bytes = std::fs::read(tmp.path()).unwrap();
    assert_eq!(tee_bytes, vec![0x05, 0x05]);
}

#[test]
fn test_container_overwrite_cli() {
    // 16-bit container: 0x1234 (binary 0001 0010 0011 0100)
    // Offset 4, width 8: bits 4..12 is 0x23 (binary 0010 0011)
    // XOR with 0xFF -> 0xDC (binary 1101 1100)
    // Updated container: 0001 1101 1100 0100 = 0x1DC4
    let mut child = Command::new(BDD_BIN)
        .args(["16[4:8] -> xor(0xFF) -> overwrite -> hex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(&[0x12, 0x34]).unwrap();
    }

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "1dc4");
}

#[test]
fn test_wildcard_passthrough_cli() {
    // 16-bit container: 0x1234
    // Pattern: 4_, 8U, 4_
    // Invert middle 8-bit unsigned integer with xor(1, 0xFF)
    let mut child = Command::new(BDD_BIN)
        .args(["4_, 8U, 4_ -> xor(1, 0xFF) -> 4_, 8U, 4_ -> hex"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bdd");

    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(&[0x12, 0x34]).unwrap();
    }

    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert_eq!(stdout.trim(), "1dc4");
}
