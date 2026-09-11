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
