//! Task 13: Intel HEX + C-array header emitters (`generators::intel_hex`).
//!
//! Ported 1:1 from `binaries.js` (`toIntelHex`/`toEsiEepromH`). The only
//! divergence: `to_intel_hex` rejects a `record.len()` not a multiple of 32
//! instead of silently reading past the array end on a partial final record
//! (the other half of the bug-5 root, see `tests/gen_eeprom.rs`).

mod common;

use soes_generator::generators::intel_hex::{to_esi_eeprom_h, to_intel_hex};

#[test]
fn intel_hex_matches_default_golden() {
    let bin = std::fs::read("tests/golden/default/eeprom.bin").unwrap();
    common::assert_eq_lines(&to_intel_hex(&bin).unwrap(), &common::load_golden("default", "eeprom.hex"));
}

#[test]
fn esi_eeprom_h_matches_default_golden() {
    let bin = std::fs::read("tests/golden/default/eeprom.bin").unwrap();
    common::assert_eq_lines(&to_esi_eeprom_h(&bin), &common::load_golden("default", "eeprom.h"));
}

#[test]
fn to_intel_hex_rejects_non_multiple_of_32() {
    assert!(to_intel_hex(&[0u8; 20]).is_err());
}

/// JS reference computes the checksum as `0x100 - (sum % 0x100)` with NO
/// further reduction mod 0x100: when a record's summed bytes are ≡ 0 (mod
/// 256), the two's-complement value is exactly 256 ("100" in hex), and
/// `.slice(-2)` takes its last 2 characters, "00" — not the single-digit
/// "0" a naive extra `% 0x100` would produce. No golden fixture happens to
/// contain a record with this property, so this is a standalone regression
/// test for that path.
#[test]
fn checksum_of_zero_sum_record_emits_two_digit_00() {
    // Record fields summed for the checksum: byte-count(0x20) + addr_hi(0x00)
    // + addr_lo(0x00) + type(0x00) = 0x20 (32). Set data[0] = 0xE0 so the
    // data sum is 224, making the full sum 32 + 224 = 256 ≡ 0 (mod 256) --
    // the exact case where JS's un-reduced `0x100 - checksum` is 256 ("100"
    // in hex), and `.slice(-2)` yields "00", not the single-digit "0" an
    // extra `% 0x100` would collapse it to.
    let mut record = [0u8; 32];
    record[0] = 0xE0;
    let hex = to_intel_hex(&record).unwrap();
    let data_line = hex.lines().next().unwrap();
    assert!(
        data_line.ends_with("00"),
        "expected two-digit '00' checksum for a sum≡0(mod 256) record, got line: {data_line}"
    );
    assert_eq!(
        data_line.len(),
        1 + 2 + 4 + 2 + 64 + 2,
        "record line must be full width including 2-digit checksum"
    );
}
