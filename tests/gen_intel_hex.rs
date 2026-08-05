//! Task 13: Intel HEX + C-array header emitters (`generators::intel_hex`).
//!
//! Ported 1:1 from `binaries.js` (`toIntelHex`/`toEsiEepromH`). The only
//! divergence: `to_intel_hex` rejects a `record.len()` not a multiple of 32
//! instead of silently reading past the array end on a partial final record
//! (the other half of the bug-5 root, see `tests/gen_eeprom.rs`).

use soes_generator::generators::intel_hex::{to_esi_eeprom_h, to_intel_hex};

#[test]
fn intel_hex_matches_default_golden() {
    let bin = std::fs::read("tests/golden/default/eeprom.bin").unwrap();
    let expected = std::fs::read_to_string("tests/golden/default/eeprom.hex").unwrap();
    assert_eq!(to_intel_hex(&bin).unwrap(), expected);
}

#[test]
fn esi_eeprom_h_matches_default_golden() {
    let bin = std::fs::read("tests/golden/default/eeprom.bin").unwrap();
    let expected = std::fs::read_to_string("tests/golden/default/eeprom.h").unwrap();
    assert_eq!(to_esi_eeprom_h(&bin), expected);
}

#[test]
fn to_intel_hex_rejects_non_multiple_of_32() {
    assert!(to_intel_hex(&[0u8; 20]).is_err());
}
