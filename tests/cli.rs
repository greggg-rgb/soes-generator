use std::process::Command;

#[test]
fn cli_emits_files_and_exits_zero() {
    let out_dir = std::env::temp_dir().join(format!(
        "soes_cli_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&out_dir);

    let output = Command::new(env!("CARGO_BIN_EXE_soes-gen"))
        .arg("tests/fixtures/default.json")
        .arg("--out")
        .arg(&out_dir)
        .output()
        .expect("failed to run soes-gen");

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let expected_files = [
        "objectlist.c",
        "utypes.h",
        "ecat_options.h",
        "eeprom.bin",
        "eeprom.hex",
        "eeprom.h",
        "esi.json",
    ];
    for name in expected_files {
        let path = out_dir.join(name);
        assert!(path.exists(), "expected file {} to exist", path.display());
    }

    let _ = std::fs::remove_dir_all(&out_dir);
}

#[test]
fn cli_bad_args_exits_nonzero() {
    let output = Command::new(env!("CARGO_BIN_EXE_soes-gen"))
        .arg("tests/fixtures/default.json")
        .output()
        .expect("failed to run soes-gen");

    assert!(!output.status.success());
}
