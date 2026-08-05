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

/// Full SII EEPROM binary image. Port of the non-`stringOnly` path of
/// `hex_generator`, `EEPROM.js:16-135`, plus its inline helpers
/// (`writeEEPROMstrings`/`writeEEPROMgeneral_settings`/`writeFMMU`/
/// `writeSyncManagers`/`FindCRC`, `EEPROM.js:138-357`).
pub fn hex_generator(config: &Config) -> Result<Vec<u8>, GenError> {
    let eeprom_size = parse_u32("EEPROMsize", &config.eeprom_size)? as usize;
    // BUG-5: the JS reference builds `new Uint8Array(EEPROMsize)` with no
    // validation at all; a size that leaves a partial final EEPROM record
    // makes the downstream .hex splitter panic ("toString of undefined").
    // Reject up front instead.
    if eeprom_size == 0 || !eeprom_size.is_multiple_of(32) {
        return Err(GenError::Config {
            field: "EEPROMsize",
            msg: format!("must be a non-zero multiple of 32 bytes (ideally 128), got {eeprom_size}"),
        });
    }

    let mut buf = vec![0xFFu8; eeprom_size];

    // WORD ADDRESS 0-7: config area (words 0-6) + CRC (word 7).
    write_config_area(&mut buf[..14], config)?;
    let crc = find_crc(&buf, 14);
    write_word(&mut buf, 7, crc as u16);

    // WORD ADDRESS 8-15: identity.
    write_dword(&mut buf, 8, parse_u32("VendorID", &config.vendor_id)?);
    write_dword(&mut buf, 10, parse_u32("ProductCode", &config.product_code)?);
    write_dword(&mut buf, 12, parse_u32("RevisionNumber", &config.revision_number)?);
    write_dword(&mut buf, 14, parse_u32("SerialNumber", &config.serial_number)?);

    // WORD ADDRESS 16-19: delay times, reserved - all zero.
    for w in 16..=19 {
        write_word(&mut buf, w, 0);
    }

    let rx_mailbox_offset = parse_u32("RxMailboxOffset", &config.rx_mailbox_offset)? as u16;
    let mailbox_size = parse_u32("MailboxSize", &config.mailbox_size)? as u16;
    let tx_mailbox_offset = parse_u32("TxMailboxOffset", &config.tx_mailbox_offset)? as u16;

    // WORD ADDRESS 20-23: bootstrap mailbox (only when FoE is enabled).
    if config.details_enable_use_foe {
        write_word(&mut buf, 20, rx_mailbox_offset);
        write_word(&mut buf, 21, mailbox_size);
        write_word(&mut buf, 22, tx_mailbox_offset);
        write_word(&mut buf, 23, mailbox_size);
    } else {
        for w in 20..=23 {
            write_word(&mut buf, w, 0);
        }
    }

    // WORD ADDRESS 24-28: standard mailbox + protocols.
    write_word(&mut buf, 24, rx_mailbox_offset);
    write_word(&mut buf, 25, mailbox_size);
    write_word(&mut buf, 26, tx_mailbox_offset);
    write_word(&mut buf, 27, mailbox_size);
    write_word(&mut buf, 28, protocols(config));
    for w in 29..=61 {
        write_word(&mut buf, w, 0);
    }
    // floor(EEPROMsize/128) - 1; signed so a sub-128-byte size (still legal
    // per the multiple-of-32 check above) matches the JS reference's
    // wraparound instead of panicking on unsigned underflow.
    let eeprom_size_word = ((eeprom_size as i64 / 128) - 1) as u16;
    write_word(&mut buf, 62, eeprom_size_word);
    write_word(&mut buf, 63, 1); // version

    // Vendor-specific info: STRING/GENERAL/FMMU/SYNCMANAGER categories.
    let strings = [
        ("TextDeviceType", config.text_device_type.as_str()),
        ("TextGroupType", config.text_group_type.as_str()),
        ("ImageName", config.image_name.as_str()),
        ("TextDeviceName", config.text_device_name.as_str()),
    ];
    for (field, s) in strings {
        check_ascii(field, s)?;
    }
    let offset = write_eeprom_strings(&mut buf, 0x80, &strings.map(|(_, s)| s));
    let offset = write_general_settings(config, offset, &mut buf);
    let offset = write_fmmu(offset, &mut buf);
    let sm2_offset = parse_u32("SM2Offset", &config.sm2_offset)? as u16;
    let sm3_offset = parse_u32("SM3Offset", &config.sm3_offset)? as u16;
    write_sync_managers(
        rx_mailbox_offset,
        mailbox_size,
        tx_mailbox_offset,
        sm2_offset,
        sm3_offset,
        offset,
        &mut buf,
    );

    Ok(buf)
}

/// Rejects any character outside the ASCII range. The JS reference writes
/// `charCodeAt` straight into a single EEPROM byte per character
/// (`writeEEPROMstrings`, `EEPROM.js:160-163`), silently truncating any
/// character above 0xFF (and misrendering 0x80-0xFF, which ETG1000.6's
/// STRING category doesn't define an encoding for) instead of erroring.
fn check_ascii(field: &'static str, s: &str) -> Result<(), GenError> {
    if let Some(c) = s.chars().find(|c| *c as u32 > 0x7F) {
        return Err(GenError::Config {
            field,
            msg: format!("non-ASCII character {c:?} is not allowed in the EEPROM STRING category"),
        });
    }
    Ok(())
}

/// `getProtocols`, `EEPROM.js:361-363`.
fn protocols(config: &Config) -> u16 {
    if config.details_enable_use_foe { 0x0C } else { 0x04 }
}

/// `getCOEdetails`, `EEPROM.js:269-279`.
fn coe_details(config: &Config) -> u8 {
    let mut d = 0u8;
    if config.coe_details_enable_sdo { d |= 0x01; }
    if config.coe_details_enable_sdo_info { d |= 0x02; }
    if config.coe_details_enable_pdo_assign { d |= 0x04; }
    if config.coe_details_enable_pdo_configuration { d |= 0x08; }
    if config.coe_details_enable_upload_at_startup { d |= 0x10; }
    if config.coe_details_enable_sdo_complete_access { d |= 0x20; }
    d
}

/// `getPhysicalPort`, `EEPROM.js:281-302`.
fn physical_port(config: &Config) -> u16 {
    let physicals = [
        config.port3_physical.as_str(),
        config.port2_physical.as_str(),
        config.port1_physical.as_str(),
        config.port0_physical.as_str(),
    ];
    let mut portinfo: u16 = 0;
    for p in physicals {
        portinfo <<= 4;
        portinfo |= match p {
            "Y" | "H" => 0x01, // MII
            "K" => 0x03,       // EBUS
            _ => 0x00,         // no connection
        };
    }
    portinfo
}

/// `writeEEPROMstrings`, `EEPROM.js:138-170` (ETG1000.6 Table 20). Returns
/// the byte offset following the STRING category.
fn write_eeprom_strings(buf: &mut [u8], offset: usize, strings: &[&str; 4]) -> usize {
    let number_of_strings = strings.len();
    let total_string_data_length =
        strings.iter().map(|s| s.len()).sum::<usize>() + number_of_strings + 1;
    let needs_pad = !total_string_data_length.is_multiple_of(2);

    write_word(buf, offset / 2, 0x000A); // Type: STRING
    write_word(buf, offset / 2 + 1, total_string_data_length.div_ceil(2) as u16);
    let mut off = offset + 4;

    buf[off] = number_of_strings as u8;
    off += 1;
    for s in strings {
        buf[off] = s.len() as u8;
        off += 1;
        buf[off..off + s.len()].copy_from_slice(s.as_bytes());
        off += s.len();
    }
    if needs_pad {
        buf[off] = 0;
        off += 1;
    }
    off
}

/// `writeEEPROMgeneral_settings`, `EEPROM.js:172-205` (ETG1000.6 Table 21).
fn write_general_settings(config: &Config, offset: usize, buf: &mut [u8]) -> usize {
    const GENERAL_CATEGORY: u16 = 0x1E;
    const CATEGORY_SIZE: u16 = 0x10;
    for wordcount in 0..(CATEGORY_SIZE as usize + 2) {
        write_word(buf, offset / 2 + wordcount, 0); // clear memory region first
    }
    write_word(buf, offset / 2, GENERAL_CATEGORY);
    write_word(buf, offset / 2 + 1, CATEGORY_SIZE);
    let mut off = offset + 4;

    buf[off] = 2; off += 1; // index to string for Group Info
    buf[off] = 3; off += 1; // index to string for Image Name
    buf[off] = 1; off += 1; // index to string for Device Order Number
    buf[off] = 4; off += 1; // index to string for Device Name Information
    off += 1; // reserved
    buf[off] = coe_details(config); off += 1;
    buf[off] = config.details_enable_use_foe as u8; off += 1; // Enable FoE
    buf[off] = 0; off += 1; // Enable EoE
    buf[off] = 0; off += 1; // reserved
    buf[off] = 0; off += 1; // reserved
    buf[off] = 0; off += 1; // reserved
    buf[off] = 0; off += 1; // flags
    write_word(buf, off / 2, 0x0000); off += 2; // current consumption in mA
    write_word(buf, off / 2, 0x0000); off += 2; // 2 pad bytes
    write_word(buf, off / 2, physical_port(config)); off += 2;
    off += 14; // pad bytes
    off
}

/// `writeFMMU`, `EEPROM.js:207-222` (ETG1000.6 Table 22).
fn write_fmmu(offset: usize, buf: &mut [u8]) -> usize {
    const FMMU_CATEGORY: u16 = 0x28;
    write_word(buf, offset / 2, FMMU_CATEGORY);
    let mut off = offset + 2;
    const LENGTH: u16 = 2; // 2 words: 3 FMMUs + padding
    write_word(buf, off / 2, LENGTH);
    off += 2;
    buf[off] = 1; off += 1; // FMMU0: Outputs
    buf[off] = 2; off += 1; // FMMU1: Inputs
    buf[off] = 3; off += 1; // FMMU2: Mailbox State
    buf[off] = 0; off += 1; // padding, disable FMMU4
    off
}

/// `writeSyncManagers`, `EEPROM.js:224-268` (ETG1000.6 Table 23).
#[allow(clippy::too_many_arguments)]
fn write_sync_managers(
    rx_mailbox_offset: u16,
    mailbox_size: u16,
    tx_mailbox_offset: u16,
    sm2_offset: u16,
    sm3_offset: u16,
    offset: usize,
    buf: &mut [u8],
) -> usize {
    const SYNC_MANAGER_CATEGORY: u16 = 0x29;
    write_word(buf, offset / 2, SYNC_MANAGER_CATEGORY);
    let mut off = offset + 2;
    write_word(buf, off / 2, 0x10); // size of structure category
    off += 2;

    // SM0
    write_word(buf, off / 2, rx_mailbox_offset); off += 2; // Physical start address
    write_word(buf, off / 2, mailbox_size); off += 2; // Physical size
    buf[off] = 0x26; off += 1; // Mode of operation
    buf[off] = 0; off += 1; // don't care
    buf[off] = 1; off += 1; // Enable Syncmanager
    buf[off] = 1; off += 1; // SyncManagerType: Mbx out

    // SM1
    write_word(buf, off / 2, tx_mailbox_offset); off += 2;
    write_word(buf, off / 2, mailbox_size); off += 2;
    buf[off] = 0x22; off += 1;
    buf[off] = 0; off += 1;
    buf[off] = 1; off += 1;
    buf[off] = 2; off += 1; // SyncManagerType: Mbx in

    // SM2
    write_word(buf, off / 2, sm2_offset); off += 2;
    write_word(buf, off / 2, 0); off += 2; // Physical size
    buf[off] = 0x24; off += 1;
    buf[off] = 0; off += 1;
    buf[off] = 1; off += 1;
    buf[off] = 3; off += 1; // SyncManagerType: PDO

    // SM3
    write_word(buf, off / 2, sm3_offset); off += 2;
    write_word(buf, off / 2, 0); off += 2;
    buf[off] = 0x20; off += 1;
    buf[off] = 0; off += 1;
    buf[off] = 1; off += 1;
    buf[off] = 4; off += 1; // SyncManagerType: PDI

    off
}

/// `FindCRC`, `EEPROM.js:305-322`: CRC-8, poly `0x07`, init `0xFF`,
/// MSB-first, over `data[..n]`.
fn find_crc(data: &[u8], n: usize) -> u8 {
    const POLY: u8 = 0x07;
    let mut crc: u8 = 0xFF;
    for &byte in &data[..n] {
        crc ^= byte;
        for _ in 0..8 {
            crc = if crc & 0x80 != 0 { (crc << 1) ^ POLY } else { crc << 1 };
        }
    }
    crc
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

/// `writeEEPROMDword_wordaddress`, `EEPROM.js:340-346`: little-endian.
fn write_dword(buf: &mut [u8], word_addr: usize, dword: u32) {
    buf[word_addr * 2..word_addr * 2 + 4].copy_from_slice(&dword.to_le_bytes());
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

    /// `find_crc` is private, so the known-answer check lives here rather
    /// than in `tests/gen_eeprom.rs`. Feeds the default golden image's
    /// first 14 bytes (the config area) and checks the result against the
    /// CRC byte the JS reference actually wrote at word 7 (byte 14) of that
    /// same golden image.
    #[test]
    fn crc8_known_answer() {
        let golden = std::fs::read("tests/golden/default/eeprom.bin")
            .unwrap_or_else(|e| panic!("missing golden default/eeprom.bin: {e}"));
        assert_eq!(find_crc(&golden, 14), golden[14]);
        assert_eq!(golden[15], 0); // CRC is 8-bit; high byte of word 7 is always 0
    }
}
