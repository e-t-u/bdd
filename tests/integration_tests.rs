use std::fs::File;
use std::io::Write;
use std::process::Command;

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
    }

    let output = child
        .wait_with_output()
        .expect("failed to wait for bdd --mcp");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 4);

    // Line 1: initialize
    assert!(lines[0].contains(&format!(
        "\"serverInfo\":{{\"name\":\"bdd-mcp\",\"version\":\"{}\"}}",
        env!("CARGO_PKG_VERSION")
    )));
    // Line 2: tools/list
    assert!(lines[1].contains("\"name\":\"bdd_slice\""));
    assert!(lines[1].contains("\"name\":\"bdd_probe\""));
    // Line 3: bdd_list_presets
    assert!(lines[2].contains("mp3-header"));
    #[cfg(feature = "small-floats")]
    assert!(lines[2].contains("nvfp4"));
    // Line 4: bdd_slice output
    assert!(lines[3].contains("sync") && lines[3].contains("2047"));
    assert!(lines[3].contains("bitrate") && lines[3].contains("9"));
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

#[cfg(feature = "server")]
#[test]
fn test_web_server_embedded() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::thread;
    use std::time::Duration;

    let port = 17892;
    let mut child = Command::new(BDD_BIN)
        .arg(format!("--serve={}", port))
        .spawn()
        .expect("failed to spawn bdd --serve");

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
        assert!(resp.contains("\"status\":\"ok\""));
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

#[cfg(not(feature = "server"))]
#[test]
fn test_web_server_disabled_error() {
    let out = Command::new(BDD_BIN)
        .arg("--serve")
        .output()
        .expect("failed to run bdd --serve");
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("The --serve web UI feature was not enabled at compile time"));
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
    // 1. Pure binary bitstream: unpack two 4-bit nibbles from each byte, swap fields, repack:
    let mut child_bin = Command::new(BDD_BIN)
        .args(["4U4U", "4U4U", "--rearrange=1,0", "--output-hex"])
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

    // 2. Text tuples: pack comma-separated pairs ("1,2") into two 4-bit nibbles per byte (0x12):
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
}
