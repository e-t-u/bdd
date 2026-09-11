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
