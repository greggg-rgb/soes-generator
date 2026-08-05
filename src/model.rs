//! Project data model: `Config` (the UI form), the object dictionary
//! (`OdSections`/`Objd`/`SubItem`), and DC sync modes — the serde shape of
//! the backup/esi.json files produced by the JS reference tool
//! (`.cache/EEPROM_generator/src/backup.js:33-51` for the top-level shape,
//! `constants.js:183-218` for every `Config` field/default).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::types::{Access, Dtype};

/// Object dictionary section: hex index string -> object definition.
/// `IndexMap` preserves JSON key order, matching the source tool's output.
pub type OdMap = IndexMap<String, Objd>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OdSections {
    #[serde(default)]
    pub sdo: OdMap,
    #[serde(default)]
    pub txpdo: OdMap,
    #[serde(default)]
    pub rxpdo: OdMap,
}

/// One CoE object dictionary entry. Source: `od.js`, `generators/objectlist.js`.
/// All three variants share the same field set — the JS reference treats
/// `objd` as one loosely-typed shape distinguished only by `otype`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "otype")]
pub enum Objd {
    #[serde(rename = "VAR")]
    Var {
        name: String,
        #[serde(default, with = "dtype_str")]
        dtype: Option<Dtype>,
        #[serde(default, with = "access_str")]
        access: Option<Access>,
        #[serde(default)]
        value: Option<serde_json::Value>,
        #[serde(default, with = "size_str")]
        size: Option<u16>,
        #[serde(default)]
        items: Vec<SubItem>,
        #[serde(default)]
        pdo_mappings: Vec<String>,
        #[serde(default, rename = "isSDOitem")]
        is_sdo_item: bool,
    },
    #[serde(rename = "ARRAY")]
    Array {
        name: String,
        #[serde(default, with = "dtype_str")]
        dtype: Option<Dtype>,
        #[serde(default, with = "access_str")]
        access: Option<Access>,
        #[serde(default)]
        value: Option<serde_json::Value>,
        #[serde(default, with = "size_str")]
        size: Option<u16>,
        #[serde(default)]
        items: Vec<SubItem>,
        #[serde(default)]
        pdo_mappings: Vec<String>,
        #[serde(default, rename = "isSDOitem")]
        is_sdo_item: bool,
    },
    #[serde(rename = "RECORD")]
    Record {
        name: String,
        #[serde(default, with = "dtype_str")]
        dtype: Option<Dtype>,
        #[serde(default, with = "access_str")]
        access: Option<Access>,
        #[serde(default)]
        value: Option<serde_json::Value>,
        #[serde(default, with = "size_str")]
        size: Option<u16>,
        #[serde(default)]
        items: Vec<SubItem>,
        #[serde(default)]
        pdo_mappings: Vec<String>,
        #[serde(default, rename = "isSDOitem")]
        is_sdo_item: bool,
    },
}

impl Objd {
    /// `Tx` if `pdo_mappings` contains `"txpdo"`, `Rx` if `"rxpdo"`, else `None`.
    pub fn pdo_dir(&self) -> crate::types::PdoDir {
        use crate::types::PdoDir;
        let pdo_mappings = match self {
            Objd::Var { pdo_mappings, .. }
            | Objd::Array { pdo_mappings, .. }
            | Objd::Record { pdo_mappings, .. } => pdo_mappings,
        };
        if pdo_mappings.iter().any(|m| m == "txpdo") {
            PdoDir::Tx
        } else if pdo_mappings.iter().any(|m| m == "rxpdo") {
            PdoDir::Rx
        } else {
            PdoDir::None
        }
    }
}

/// A `RECORD`/`ARRAY` sub-index entry, e.g. `{"name": "Max SubIndex"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubItem {
    pub name: String,
    #[serde(default, with = "dtype_str")]
    pub dtype: Option<Dtype>,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    #[serde(default, with = "access_str")]
    pub access: Option<Access>,
}

/// The UI form / device settings. Field list and defaults transcribed
/// verbatim from `getFormDefaultValues()`, `constants.js:183-218`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "VendorName")]
    pub vendor_name: String,
    #[serde(rename = "VendorID")]
    pub vendor_id: String,
    #[serde(rename = "ProductCode")]
    pub product_code: String,
    #[serde(rename = "ProfileNo")]
    pub profile_no: String,
    #[serde(rename = "RevisionNumber")]
    pub revision_number: String,
    #[serde(rename = "SerialNumber")]
    pub serial_number: String,
    #[serde(rename = "HWversion")]
    pub hw_version: String,
    #[serde(rename = "SWversion")]
    pub sw_version: String,
    #[serde(rename = "EEPROMsize")]
    pub eeprom_size: String,
    #[serde(rename = "RxMailboxOffset")]
    pub rx_mailbox_offset: String,
    #[serde(rename = "TxMailboxOffset")]
    pub tx_mailbox_offset: String,
    #[serde(rename = "MailboxSize")]
    pub mailbox_size: String,
    #[serde(rename = "SM2Offset")]
    pub sm2_offset: String,
    #[serde(rename = "SM3Offset")]
    pub sm3_offset: String,
    #[serde(rename = "TextGroupType")]
    pub text_group_type: String,
    #[serde(rename = "TextGroupName5")]
    pub text_group_name5: String,
    #[serde(rename = "ImageName")]
    pub image_name: String,
    #[serde(rename = "TextDeviceType")]
    pub text_device_type: String,
    #[serde(rename = "TextDeviceName")]
    pub text_device_name: String,
    #[serde(rename = "Port0Physical")]
    pub port0_physical: String,
    #[serde(rename = "Port1Physical")]
    pub port1_physical: String,
    #[serde(rename = "Port2Physical")]
    pub port2_physical: String,
    #[serde(rename = "Port3Physical")]
    pub port3_physical: String,
    #[serde(rename = "ESC")]
    pub esc: String,
    #[serde(rename = "SPImode")]
    pub spi_mode: String,
    #[serde(rename = "CoeDetailsEnableSDO", default)]
    pub coe_details_enable_sdo: bool,
    #[serde(rename = "CoeDetailsEnableSDOInfo", default)]
    pub coe_details_enable_sdo_info: bool,
    #[serde(rename = "CoeDetailsEnablePDOAssign", default)]
    pub coe_details_enable_pdo_assign: bool,
    #[serde(rename = "CoeDetailsEnablePDOConfiguration", default)]
    pub coe_details_enable_pdo_configuration: bool,
    #[serde(rename = "CoeDetailsEnableUploadAtStartup", default)]
    pub coe_details_enable_upload_at_startup: bool,
    #[serde(rename = "CoeDetailsEnableSDOCompleteAccess", default)]
    pub coe_details_enable_sdo_complete_access: bool,
    #[serde(rename = "DetailsEnableUseFoE", default)]
    pub details_enable_use_foe: bool,
}

/// One DC (distributed clocks) sync mode entry. Renames match the backup
/// `dc[]` keys verbatim (source data captured from the UI, not JS constants).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncMode {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "AssignActivate")]
    pub assign_activate: String,
    #[serde(rename = "Sync0cycleTime")]
    pub sync0_cycle_time: String,
    #[serde(rename = "Sync0shiftTime")]
    pub sync0_shift_time: String,
    #[serde(rename = "Sync1cycleTime")]
    pub sync1_cycle_time: String,
    #[serde(rename = "Sync1shiftTime")]
    pub sync1_shift_time: String,
}

/// The full project: form settings + object dictionary + DC sync modes.
/// Top-level shape matches `prepareBackupObject`, `backup.js:33-51`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(rename = "form")]
    pub config: Config,
    #[serde(rename = "od")]
    pub od: OdSections,
    #[serde(default)]
    pub dc: Vec<SyncMode>,
}

impl Project {
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn from_json_file(p: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let s = std::fs::read_to_string(p)?;
        Ok(Self::from_json(&s)?)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("Project serialization is infallible")
    }
}

/// (De)serializes `Option<Dtype>` as its JS `DTYPE_*` macro-suffix string
/// (e.g. `"UNSIGNED8"`), via `Dtype::from_ident`/`macro_suffix`.
mod dtype_str {
    use crate::types::Dtype;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<Dtype>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(d) => s.serialize_str(d.macro_suffix()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Dtype>, D::Error> {
        match Option::<String>::deserialize(d)? {
            Some(s) => Dtype::from_ident(&s)
                .map(Some)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown dtype {s:?}"))),
            None => Ok(None),
        }
    }
}

/// (De)serializes `Option<Access>` as its `RO`/`RW`/`WO`/`RWpre` string, via
/// `Access::from_str`/`macro_suffix`.
mod access_str {
    use crate::types::Access;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<Access>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(a) => s.serialize_str(a.macro_suffix()),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Access>, D::Error> {
        match Option::<String>::deserialize(d)? {
            Some(s) => Access::from_str(&s)
                .map(Some)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown access {s:?}"))),
            None => Ok(None),
        }
    }
}

/// (De)serializes `Option<u16>`. The real backup format writes `objd.size`
/// as a JSON **number** (`ui.js:453` `objd.size = parseInt(...)`,
/// `od.js:270-274`); some Jasmine spec literals use a string instead. Accept
/// either on read (mirrors the tolerant handling already used for `value`),
/// always emit a plain number on write.
mod size_str {
    use serde::{Deserialize, Deserializer, Serializer};
    use serde_json::Value;

    pub fn serialize<S: Serializer>(v: &Option<u16>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(n) => s.serialize_u16(*n),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u16>, D::Error> {
        match Option::<Value>::deserialize(d)? {
            Some(Value::Number(n)) => n
                .as_u64()
                .and_then(|v| u16::try_from(v).ok())
                .map(Some)
                .ok_or_else(|| serde::de::Error::custom(format!("size out of range: {n}"))),
            Some(Value::String(s)) => s
                .trim()
                .parse::<u16>()
                .map(Some)
                .map_err(serde::de::Error::custom),
            Some(other) => Err(serde::de::Error::custom(format!(
                "invalid size: {other}"
            ))),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the real backup format writes `objd.size` as a JSON
    /// number (`ui.js:453`, `od.js:270-274`), not a string — a numeric
    /// VISIBLE_STRING `size` must deserialize, not hard-fail.
    #[test]
    fn size_accepts_numeric_and_string() {
        let numeric = r#"{"otype":"VAR","name":"Device name","dtype":"VISIBLE_STRING","size":8}"#;
        let o: Objd = serde_json::from_str(numeric).expect("numeric size");
        let Objd::Var { size, .. } = o else { panic!("expected Var") };
        assert_eq!(size, Some(8));

        let stringy = r#"{"otype":"VAR","name":"Device name","dtype":"VISIBLE_STRING","size":"8"}"#;
        let o: Objd = serde_json::from_str(stringy).expect("string size");
        let Objd::Var { size, .. } = o else { panic!("expected Var") };
        assert_eq!(size, Some(8));
    }
}
