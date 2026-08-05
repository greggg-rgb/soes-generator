//! Task 12: full SII EEPROM binary image (`eeprom::hex_generator`).
//!
//! `find_crc` is private (ported 1:1 from `FindCRC`, `EEPROM.js:305-322`) and
//! is unit-tested in-module (`src/generators/eeprom.rs`); the per-ESC
//! `config_data_string` golden coverage lives there too
//! (`config_data_matches_all_esc_goldens`). This file covers the public,
//! full-image surface: the byte-for-byte golden match (the highest-risk
//! assertion in the whole port), EEPROMsize validation (BUG-5), and
//! non-ASCII rejection in the EEPROM STRING category.

use soes_generator::{generators::eeprom, model::Project, GenError};

fn load_fixture(name: &str) -> Project {
    Project::from_json(&std::fs::read_to_string(format!("tests/fixtures/{name}.json")).unwrap())
        .unwrap()
}

#[test]
fn full_image_matches_default_golden() {
    let p = load_fixture("default");
    let out = eeprom::hex_generator(&p.config).unwrap();
    assert_eq!(out, std::fs::read("tests/golden/default/eeprom.bin").unwrap());
}

#[test]
fn full_image_matches_cia402_golden() {
    let p = load_fixture("cia402");
    let out = eeprom::hex_generator(&p.config).unwrap();
    assert_eq!(out, std::fs::read("tests/golden/cia402/eeprom.bin").unwrap());
}

#[test]
fn full_image_matches_foe_golden() {
    let p = load_fixture("foe");
    let out = eeprom::hex_generator(&p.config).unwrap();
    assert_eq!(out, std::fs::read("tests/golden/foe/eeprom.bin").unwrap());
}

/// BUG-5: the JS reference builds `record = new Uint8Array(EEPROMsize)` and
/// happily accepts any size, which downstream (the .hex splitter) panics on
/// with "toString of undefined" for a partial final record. We instead
/// reject non-multiple-of-32 sizes up front.
#[test]
fn eepromsize_not_multiple_of_32_errors() {
    let mut p = load_fixture("default");
    p.config.eeprom_size = "2000".into();
    match eeprom::hex_generator(&p.config).unwrap_err() {
        GenError::Config { field, .. } => assert_eq!(field, "EEPROMsize"),
        other => panic!("expected GenError::Config{{field: \"EEPROMsize\"}}, got {other:?}"),
    }
}

/// BUG-5 (fix round 1): a 32-aligned size can still be too small to hold
/// the fixed config/identity/mailbox area plus the STRING/GENERAL/FMMU/
/// SYNCMANAGER categories - e.g. 128 is exactly where the STRING category
/// starts, leaving no room for it at all. This must return an error, not
/// panic with an out-of-bounds write.
#[test]
fn eepromsize_aligned_but_too_small_errors() {
    let mut p = load_fixture("default");
    p.config.eeprom_size = "128".into();
    match eeprom::hex_generator(&p.config).unwrap_err() {
        GenError::Config { field, .. } => assert_eq!(field, "EEPROMsize"),
        other => panic!("expected GenError::Config{{field: \"EEPROMsize\"}}, got {other:?}"),
    }
}

/// The JS reference writes `charCodeAt` straight into a single EEPROM byte
/// per character (`writeEEPROMstrings`, `EEPROM.js:138-170`), silently
/// truncating any character above 0xFF and corrupting the STRING category.
/// We reject non-ASCII input in the four device/vendor strings that feed it
/// instead of emitting a corrupt image.
#[test]
fn non_ascii_device_string_is_rejected() {
    let mut p = load_fixture("default");
    p.config.text_device_name = "Motörhead".into();
    match eeprom::hex_generator(&p.config).unwrap_err() {
        GenError::Config { field, .. } => assert_eq!(field, "TextDeviceName"),
        other => panic!("expected GenError::Config, got {other:?}"),
    }
}

/// Sweeps every 32-aligned EEPROMsize from 0 to 2048: `hex_generator` must
/// always return `Ok` or `Err(GenError::Config{field:"EEPROMsize",..})`,
/// never panic. Directly covers the bug report ("128, 160, ... 256 all pass
/// the multiple-of-32 check yet panic").
#[test]
fn eepromsize_never_panics_across_aligned_sizes() {
    let p = load_fixture("default");
    for size in (0..=2048usize).step_by(32) {
        let mut config = p.config.clone();
        config.eeprom_size = size.to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            eeprom::hex_generator(&config)
        }));
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(GenError::Config { field, .. })) => assert_eq!(field, "EEPROMsize"),
            Ok(Err(other)) => panic!("size {size}: expected Config error, got {other:?}"),
            Err(_) => panic!("size {size}: hex_generator panicked"),
        }
    }
}

#[test]
fn ascii_device_strings_are_accepted() {
    let p = load_fixture("default");
    assert!(eeprom::hex_generator(&p.config).is_ok());
}
