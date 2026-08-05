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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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

    fn pdo_mappings_mut(&mut self) -> &mut Vec<String> {
        match self {
            Objd::Var { pdo_mappings, .. }
            | Objd::Array { pdo_mappings, .. }
            | Objd::Record { pdo_mappings, .. } => pdo_mappings,
        }
    }

    /// Builds a `VAR` entry. Index -> uppercase-hex key (`format!("{index:X}")`).
    pub fn var(index: u16, dtype: Dtype, name: impl Into<String>) -> (String, Objd) {
        (
            format!("{index:X}"),
            Objd::Var {
                name: name.into(),
                dtype: Some(dtype),
                access: None,
                value: None,
                size: None,
                items: Vec::new(),
                pdo_mappings: Vec::new(),
                is_sdo_item: false,
            },
        )
    }

    /// Builds a `VAR` entry with an initial value (used by REAL32/REAL64
    /// value tests etc.). `value` is stored as a JSON string, matching the
    /// backup format's `objd.value` (e.g. `"value": "1.5"`).
    pub fn var_with_value(
        index: u16,
        dtype: Dtype,
        name: impl Into<String>,
        value: &str,
    ) -> (String, Objd) {
        let (key, mut objd) = Objd::var(index, dtype, name);
        set_value(&mut objd, value);
        (key, objd)
    }

    /// Builds a `VISIBLE_STRING` `VAR` entry with an initial value and size.
    pub fn var_string(index: u16, name: impl Into<String>, value: &str, size: u16) -> (String, Objd) {
        let (key, mut objd) = Objd::var(index, Dtype::VisibleString, name);
        set_value(&mut objd, value);
        if let Objd::Var { size: s, .. } = &mut objd {
            *s = Some(size);
        }
        (key, objd)
    }

    /// Builds an `ARRAY` entry. `dtype` is the element type shared by every
    /// sub-item (matches the JS reference, e.g. `constants.js:142`'s Sync
    /// Manager Communication Type array).
    pub fn array(
        index: u16,
        dtype: Dtype,
        name: impl Into<String>,
        items: Vec<SubItem>,
    ) -> (String, Objd) {
        (
            format!("{index:X}"),
            Objd::Array {
                name: name.into(),
                dtype: Some(dtype),
                access: None,
                value: None,
                size: None,
                items,
                pdo_mappings: Vec::new(),
                is_sdo_item: false,
            },
        )
    }

    /// Builds a `RECORD` entry. No top-level `dtype` — each sub-item carries
    /// its own (matches the JS reference, e.g. Identity Object/Sync Manager
    /// Parameters records in `constants.js`/`od.js`).
    pub fn record(index: u16, name: impl Into<String>, items: Vec<SubItem>) -> (String, Objd) {
        (
            format!("{index:X}"),
            Objd::Record {
                name: name.into(),
                dtype: None,
                access: None,
                value: None,
                size: None,
                items,
                pdo_mappings: Vec::new(),
                is_sdo_item: false,
            },
        )
    }
}

fn set_value(objd: &mut Objd, value: &str) {
    let v = Some(serde_json::Value::String(value.to_string()));
    match objd {
        Objd::Var { value: dst, .. } | Objd::Array { value: dst, .. } | Objd::Record { value: dst, .. } => {
            *dst = v;
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

impl Config {
    /// `getFormDefaultValues().form`, transcribed verbatim from
    /// `constants.js:183-218`. This is the seed `ProjectBuilder` uses —
    /// NOT `Default::default()` (which would be empty strings and break
    /// `build_object_dictionary`'s `parse_u32` on the identity fields).
    pub fn form_defaults() -> Config {
        Config {
            vendor_name: "ACME EtherCAT Devices".into(),
            vendor_id: "0x000".into(),
            product_code: "0x00ab123".into(),
            profile_no: "5001".into(),
            revision_number: "0x002".into(),
            serial_number: "0x001".into(),
            hw_version: "0.0.1".into(),
            sw_version: "0.0.1".into(),
            eeprom_size: "2048".into(),
            rx_mailbox_offset: "0x1000".into(),
            tx_mailbox_offset: "0x1200".into(),
            mailbox_size: "512".into(),
            sm2_offset: "0x1400".into(),
            sm3_offset: "0x1A00".into(),
            text_group_type: "DigIn".into(),
            text_group_name5: "Digital input".into(),
            image_name: "IMGCBY".into(),
            text_device_type: "DigIn2000".into(),
            text_device_name: "2-channel Hypergalactic input superimpermanator".into(),
            port0_physical: "Y".into(),
            port1_physical: "Y".into(),
            port2_physical: " ".into(),
            port3_physical: " ".into(),
            esc: "ET1100".into(),
            spi_mode: "3".into(),
            coe_details_enable_sdo: true,
            coe_details_enable_sdo_info: true,
            coe_details_enable_pdo_assign: false,
            coe_details_enable_pdo_configuration: false,
            coe_details_enable_upload_at_startup: true,
            coe_details_enable_sdo_complete_access: false,
            details_enable_use_foe: false,
        }
    }
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

    /// Starts a `ProjectBuilder` seeded with the JS form defaults
    /// (`Config::form_defaults`) and an empty object dictionary/DC list.
    pub fn builder() -> ProjectBuilder {
        ProjectBuilder {
            config: Config::form_defaults(),
            od: OdSections::default(),
            dc: Vec::new(),
        }
    }
}

/// Fluent constructor for `Project`. `config` is public — override fields
/// directly (`b.config.vendor_id = "0x123".into()`) — and pre-seeded with
/// the JS form defaults, not empty strings (see `Config::form_defaults`).
pub struct ProjectBuilder {
    pub config: Config,
    od: OdSections,
    dc: Vec<SyncMode>,
}

impl ProjectBuilder {
    fn insert(mut self, (index, mut objd): (String, Objd), pdo_mapping: Option<&str>) -> Self {
        let section = match pdo_mapping {
            Some(m) => {
                objd.pdo_mappings_mut().push(m.to_string());
                match m {
                    "txpdo" => &mut self.od.txpdo,
                    "rxpdo" => &mut self.od.rxpdo,
                    _ => unreachable!("only txpdo/rxpdo are used as pdo_mapping tags"),
                }
            }
            None => &mut self.od.sdo,
        };
        section.insert(index, objd);
        self
    }

    /// Adds `o` to the SDO section (no PDO mapping tag).
    pub fn add_sdo(self, o: (String, Objd)) -> Self {
        self.insert(o, None)
    }

    /// Adds `o` to the TxPDO section and tags it `pdo_mappings: ["txpdo"]`.
    pub fn add_txpdo(self, o: (String, Objd)) -> Self {
        self.insert(o, Some("txpdo"))
    }

    /// Adds `o` to the RxPDO section and tags it `pdo_mappings: ["rxpdo"]`.
    pub fn add_rxpdo(self, o: (String, Objd)) -> Self {
        self.insert(o, Some("rxpdo"))
    }

    pub fn build(self) -> Project {
        Project {
            config: self.config,
            od: self.od,
            dc: self.dc,
        }
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
