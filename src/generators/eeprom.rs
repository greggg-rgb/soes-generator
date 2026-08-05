//! Config-data helpers shared by the ESI XML `<ConfigData>` embed (Task 11)
//! and the full EEPROM binary image (Task 12). Port of the "config area"
//! slice of `.cache/EEPROM_generator/src/generators/EEPROM.js`
//! (`getConfigDataBytes` + `getConfigDataString`, the `hex_generator(form,
//! true)` / `stringOnly` path, `EEPROM.js:16-73,348-357`).
//!
//! Only word addresses 0-6 (bytes 0-13) are built here. Word 7 (the CRC,
//! `FindCRC`, `EEPROM.js:305-322`) and every category/string past byte 13
//! belong to the full EEPROM image and are Task 12's job — `config_data_string`
//! never needs them (`getConfigDataString` only ever reads `record[0..14]`).

use crate::model::Config;
use crate::names::parse_u32;
use crate::types::Esc;
use crate::GenError;

/// `getConfigDataString`, `EEPROM.js:349-357` (the `hex_generator(form,
/// true)`/`stringOnly` path). Byte count: 14 (28 hex chars) for ESCs that
/// use the reserved bytes for configuration (`Esc::config_on_reserved_bytes`,
/// `configOnReservedBytes` in `constants.js`), else 7 (14 hex chars).
pub fn config_data_string(config: &Config) -> Result<String, GenError> {
    let esc = parse_esc(config)?;

    let mut buf = [0u8; 14];
    write_config_area(&mut buf, config)?;

    let n = if esc.config_on_reserved_bytes() { 14 } else { 7 };
    Ok(buf[..n].iter().map(|b| format!("{b:02X}")).collect())
}

/// `getConfigDataBytes`, `EEPROM.js:24-73` — word addresses 0-6 (bytes
/// 0-13) only. `buf` must be at least 14 bytes long; only `buf[0..14]` is
/// written (Task 12's full-image `hex_generator` reuses this to fill the
/// start of a larger buffer, then adds the CRC at word 7 itself).
fn write_config_area(buf: &mut [u8], config: &Config) -> Result<(), GenError> {
    let esc = parse_esc(config)?;
    let spi_mode = parse_u32("SPImode", &config.spi_mode)? as u8;

    buf[0] = esc.pdi_control(); // PDI control: SPI slave (register 0x0140)
    buf[1] = 0x06; // ESC configuration: DC Sync Out/Latch In enabled (0x0141)
    buf[2] = spi_mode; // SPI mode (register 0x0150)
    buf[3] = 0x44; // SYNC/LATCH configuration (0x0151): both Syncs output
    write_word(buf, 2, 0x0064); // Syncsignal pulse length, 10ns units (0x0982:0x0983)
    write_word(buf, 3, 0x0000); // Extended PDI configuration (none for SPI slave) (0x0152:0x0153)
    write_word(buf, 4, 0x0000); // Configured Station Alias (0x0012:0x0013)
    write_word(buf, 5, esc.reserved_0x05()); // Reserved (ESC-specific config)
    write_word(buf, 6, 0x0000); // Reserved, 0

    Ok(())
}

fn parse_esc(config: &Config) -> Result<Esc, GenError> {
    Esc::from_str(&config.esc).ok_or_else(|| GenError::Config {
        field: "ESC",
        msg: format!("unknown ESC {:?}", config.esc),
    })
}

/// `writeEEPROMword_wordaddress`, `EEPROM.js:334-338`: little-endian, byte
/// offset = word address * 2.
fn write_word(buf: &mut [u8], word_addr: usize, word: u16) {
    buf[word_addr * 2] = (word & 0xFF) as u8;
    buf[word_addr * 2 + 1] = (word >> 8) as u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_esc(esc: &str) -> Config {
        let mut c = Config::form_defaults();
        c.esc = esc.into();
        c
    }

    #[test]
    fn config_data_matches_all_esc_goldens() {
        let cases = [
            ("ET1100", "et1100.txt"),
            ("AX58100", "ax58100.txt"),
            ("LAN9252", "lan9252.txt"),
            ("LAN9253 Beckhoff", "lan9253_beckhoff.txt"),
            ("LAN9253 Direct", "lan9253_direct.txt"),
            ("LAN9253 Indirect", "lan9253_indirect.txt"),
        ];
        for (esc, file) in cases {
            let config = config_with_esc(esc);
            let golden = std::fs::read_to_string(format!("tests/golden/esc/{file}"))
                .unwrap_or_else(|e| panic!("missing golden {file}: {e}"));
            assert_eq!(
                config_data_string(&config).unwrap(),
                golden.trim(),
                "ESC {esc}"
            );
        }
    }
}
