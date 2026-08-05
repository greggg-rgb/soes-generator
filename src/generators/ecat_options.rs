//! `ecat_options.h` generator — port of
//! `.cache/EEPROM_generator/src/generators/ecat_options.js`
//! (`ecat_options_generator`, whole file).
//!
//! Straight `#define` dump: mailbox/SM addresses come from `Config` (offsets
//! formatted via `index_to_string`, mirroring JS `indexToString` —
//! `parseInt` then `toString(16).toUpperCase()`, i.e. uppercase hex with no
//! `0x` prefix, prefix added at each call site). `MAX_MAPPINGS_SM2`/`_SM3`
//! come from `get_max_mappings` (`getMaxMappings`, ecat_options.js:76-100).
//! `MAX_{RX,TX}PDO_SIZE` stay hard-coded at 512 — a reference limitation
//! (`ecat_options.js`'s own `// TODO calculate based on offset, size`).

use crate::model::Config;
use crate::names::parse_u32;
use crate::od_build::Od;
use crate::types::Dtype;

pub fn generate(config: &Config, od: &Od) -> String {
    let use_foe = if config.details_enable_use_foe { 1 } else { 0 };
    // These `.expect()`s are safe only because `eeprom::hex_generator` runs
    // earlier in `generate()` (lib.rs) and validates these same MailboxSize/
    // Rx/TxMailboxOffset/SM2/SM3Offset fields via `?` first — a malformed
    // value already returned a clean GenError before this infallible
    // (`-> String`) function is ever reached.
    let mailbox_size = parse_u32("MailboxSize", &config.mailbox_size).expect("valid MailboxSize");
    let rx_mailbox = index_to_string(&config.rx_mailbox_offset, "RxMailboxOffset");
    let tx_mailbox = index_to_string(&config.tx_mailbox_offset, "TxMailboxOffset");
    let sm2 = index_to_string(&config.sm2_offset, "SM2Offset");
    let sm3 = index_to_string(&config.sm3_offset, "SM3Offset");

    let max_mappings_sm2 = get_max_mappings(od, "rxpdo");
    let max_mappings_sm3 = get_max_mappings(od, "txpdo");

    format!(
        "#ifndef __ECAT_OPTIONS_H__\n#define __ECAT_OPTIONS_H__\n\n\
         #define USE_FOE          {use_foe}\n#define USE_EOE          0\n\n\
         #define MBXSIZE          {mailbox_size}\n#define MBXSIZEBOOT      {mailbox_size}\n#define MBXBUFFERS       3\n\n\
         #define MBX0_sma         0x{rx_mailbox}\n#define MBX0_sml         MBXSIZE\n#define MBX0_sme         MBX0_sma+MBX0_sml-1\n#define MBX0_smc         0x26\n\
         #define MBX1_sma         0x{tx_mailbox}\n#define MBX1_sml         MBXSIZE\n#define MBX1_sme         MBX1_sma+MBX1_sml-1\n#define MBX1_smc         0x22\n\n\
         #define MBX0_sma_b       0x{rx_mailbox}\n#define MBX0_sml_b       MBXSIZEBOOT\n#define MBX0_sme_b       MBX0_sma_b+MBX0_sml_b-1\n#define MBX0_smc_b       0x26\n\
         #define MBX1_sma_b       0x{tx_mailbox}\n#define MBX1_sml_b       MBXSIZEBOOT\n#define MBX1_sme_b       MBX1_sma_b+MBX1_sml_b-1\n#define MBX1_smc_b       0x22\n\n\
         #define SM2_sma          0x{sm2}\n#define SM2_smc          0x24\n#define SM2_act          1\n\
         #define SM3_sma          0x{sm3}\n#define SM3_smc          0x20\n#define SM3_act          1\n\n\
         #define MAX_MAPPINGS_SM2 {max_mappings_sm2}\n#define MAX_MAPPINGS_SM3 {max_mappings_sm3}\n\n\
         #define MAX_RXPDO_SIZE   512\n#define MAX_TXPDO_SIZE   512\n\n\
         #endif /* __ECAT_OPTIONS_H__ */\n"
    )
}

/// `indexToString`, `od.js:295-298`: `parseInt` then `toString(16).toUpperCase()`
/// — uppercase hex, no `0x` prefix (callers add it).
fn index_to_string(s: &str, field: &'static str) -> String {
    format!("{:X}", parse_u32(field, s).expect("valid offset"))
}

/// `getMaxMappings`, `ecat_options.js:76-100`. Counts PDO-mapped subitems
/// for `pdo_name` ("rxpdo" -> SM2, "txpdo" -> SM3, ecat_options.js:76-77),
/// +1 extra per BOOLEAN (padding entry is a mapping too).
fn get_max_mappings(od: &Od, pdo_name: &str) -> u32 {
    let mut result = 0u32;
    for objd in od.values() {
        let items = objd.items();
        if !items.is_empty() {
            // ARRAY/RECORD: top-level pdo_mappings gates every real subitem
            // (subindex 0, "Max SubIndex", is skipped).
            for subitem in items.iter().skip(1) {
                for mapping in objd.pdo_mappings() {
                    if mapping == pdo_name {
                        result += 1;
                        if subitem.dtype == Some(Dtype::Boolean) {
                            result += 1;
                        }
                    }
                }
            }
        } else {
            for mapping in objd.pdo_mappings() {
                if mapping == pdo_name {
                    result += 1;
                    if objd.dtype() == Some(Dtype::Boolean) {
                        result += 1;
                    }
                }
            }
        }
    }
    result
}
