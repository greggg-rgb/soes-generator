//! Intel HEX + C-array header emitters.
//!
//! Ported 1:1 from `binaries.js` (`toIntelHex`/`toEsiEepromH`).

use crate::GenError;

const BYTES_PER_RECORD: usize = 32;

/// Encodes `record` as Intel HEX: 32 bytes per data record, big-endian
/// 4-hex-digit address, record type `00`, two's-complement checksum byte,
/// followed by the `:00000001FF` EOF record. Entire output is uppercased.
///
/// Errors if `record.len()` is not a multiple of 32 bytes — the JS
/// reference (`binaries.js:17`) reads past the array end on a partial
/// final record instead of rejecting it.
pub fn to_intel_hex(record: &[u8]) -> Result<String, GenError> {
    if !record.len().is_multiple_of(BYTES_PER_RECORD) {
        return Err(GenError::Config {
            field: "record",
            msg: format!(
                "length {} is not a multiple of {BYTES_PER_RECORD}",
                record.len()
            ),
        });
    }

    let mut hex = String::new();
    for (rulenumber, chunk) in record.chunks(BYTES_PER_RECORD).enumerate() {
        let address = (rulenumber * BYTES_PER_RECORD) as u16;
        // record type is always '00' (data record), so it contributes 0 to
        // the checksum and is omitted from the running sum below.
        let mut checksum: u32 = BYTES_PER_RECORD as u32 + (address >> 8) as u32 + (address & 0xff) as u32;
        hex.push(':');
        hex.push_str(&format!("{BYTES_PER_RECORD:02x}{address:04x}00"));
        for &byte in chunk {
            hex.push_str(&format!("{byte:02x}"));
            checksum += byte as u32;
        }
        let checksum = (0x100 - (checksum % 0x100)) % 0x100; // two's complement
        // JS reference: `checksum.toString(16).slice(-2)` does NOT zero-pad —
        // a checksum below 0x10 (unlike data bytes, which pad explicitly)
        // emits as a single hex digit. Replicated verbatim for golden parity.
        hex.push_str(&format!("{checksum:x}\n"));
    }
    hex.push_str(":00000001FF");
    Ok(hex.to_uppercase())
}

/// Encodes `record` as the `esi.h` C header: 16 bytes per line as `0xNN`,
/// comma-separated, wrapped in the `esiEepromData[]` array.
///
/// The trailing `#endif __ESI_EEPROM_H__` (bare token, not a comment) is
/// emitted verbatim to match `binaries.js:71` byte-for-byte — do not "fix"
/// it to valid C.
pub fn to_esi_eeprom_h(record: &[u8]) -> String {
    let mut result = String::from(
        "#ifndef __ESI_EEPROM_H__\n#define __ESI_EEPROM_H__\n\nunsigned char esiEepromData[] = {\n",
    );
    let mut line = 0;
    let last_i = record.len().wrapping_sub(1);
    for (i, &b) in record.iter().enumerate() {
        result.push_str(&format!("0x{b:02X}"));
        line += 1;
        if i >= last_i {
            break;
        }
        if line >= 16 {
            result.push_str(",\n");
            line = 0;
        } else {
            result.push(',');
        }
    }
    result.push_str("\n};\n#endif __ESI_EEPROM_H__");
    result
}
