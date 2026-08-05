//! C-ABI source of truth: CoE data/object types, access modes, PDO direction,
//! and supported EtherCAT slave controllers (ESC), transcribed verbatim from
//! the JS reference generator (`.cache/EEPROM_generator/src/constants.js`,
//! `.cache/EEPROM_generator/src/generators/EEPROM.js`).

/// CoE data type. Variant order/names mirror `DTYPE` in `constants.js:26-45`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dtype {
    Boolean,
    Integer8,
    Integer16,
    Integer32,
    Integer64,
    Real32,
    Real64,
    Unsigned8,
    Unsigned16,
    Unsigned32,
    Unsigned64,
    VisibleString,
}

/// ESI XML data type row: IEC 61131-3 name, bit size, and C type.
/// Transcribed from `ESI_DT` in `constants.js:78-91`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EsiType {
    pub iec_name: &'static str,
    pub bitsize: u16,
    pub ctype: &'static str,
}

impl Dtype {
    /// ESI XML row for this dtype. Source: `constants.js:78-91`.
    pub fn esi(&self) -> EsiType {
        match self {
            Dtype::Boolean => EsiType { iec_name: "BOOL", bitsize: 1, ctype: "uint8_t" },
            Dtype::Integer8 => EsiType { iec_name: "SINT", bitsize: 8, ctype: "int8_t" },
            Dtype::Integer16 => EsiType { iec_name: "INT", bitsize: 16, ctype: "int16_t" },
            Dtype::Integer32 => EsiType { iec_name: "DINT", bitsize: 32, ctype: "int32_t" },
            Dtype::Integer64 => EsiType { iec_name: "LINT", bitsize: 64, ctype: "int64_t" },
            Dtype::Real32 => EsiType { iec_name: "REAL", bitsize: 32, ctype: "float" },
            Dtype::Real64 => EsiType { iec_name: "LREAL", bitsize: 64, ctype: "double" },
            Dtype::Unsigned8 => EsiType { iec_name: "USINT", bitsize: 8, ctype: "uint8_t" },
            Dtype::Unsigned16 => EsiType { iec_name: "UINT", bitsize: 16, ctype: "uint16_t" },
            Dtype::Unsigned32 => EsiType { iec_name: "UDINT", bitsize: 32, ctype: "uint32_t" },
            Dtype::Unsigned64 => EsiType { iec_name: "ULINT", bitsize: 64, ctype: "uint64_t" },
            Dtype::VisibleString => EsiType { iec_name: "STRING", bitsize: 8, ctype: "char" },
        }
    }

    /// JS `DTYPE` enum string, used to build `DTYPE_*` macros. Source: `constants.js:26-45`.
    pub fn macro_suffix(&self) -> &'static str {
        match self {
            Dtype::Boolean => "BOOLEAN",
            Dtype::Integer8 => "INTEGER8",
            Dtype::Integer16 => "INTEGER16",
            Dtype::Integer32 => "INTEGER32",
            Dtype::Integer64 => "INTEGER64",
            Dtype::Real32 => "REAL32",
            Dtype::Real64 => "REAL64",
            Dtype::Unsigned8 => "UNSIGNED8",
            Dtype::Unsigned16 => "UNSIGNED16",
            Dtype::Unsigned32 => "UNSIGNED32",
            Dtype::Unsigned64 => "UNSIGNED64",
            Dtype::VisibleString => "VISIBLE_STRING",
        }
    }

    /// Parses a `DTYPE_*` macro-suffix string back into a `Dtype`.
    pub fn from_ident(s: &str) -> Option<Dtype> {
        match s {
            "BOOLEAN" => Some(Dtype::Boolean),
            "INTEGER8" => Some(Dtype::Integer8),
            "INTEGER16" => Some(Dtype::Integer16),
            "INTEGER32" => Some(Dtype::Integer32),
            "INTEGER64" => Some(Dtype::Integer64),
            "REAL32" => Some(Dtype::Real32),
            "REAL64" => Some(Dtype::Real64),
            "UNSIGNED8" => Some(Dtype::Unsigned8),
            "UNSIGNED16" => Some(Dtype::Unsigned16),
            "UNSIGNED32" => Some(Dtype::Unsigned32),
            "UNSIGNED64" => Some(Dtype::Unsigned64),
            "VISIBLE_STRING" => Some(Dtype::VisibleString),
            _ => None,
        }
    }
}

/// CoE object type. Source: `OTYPE` in `constants.js:20-24`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Otype {
    Var,
    Array,
    Record,
}

impl Otype {
    pub fn macro_suffix(&self) -> &'static str {
        match self {
            Otype::Var => "VAR",
            Otype::Array => "ARRAY",
            Otype::Record => "RECORD",
        }
    }
}

/// Object access mode. Values `RO`/`RW`/`WO`/`RWpre` per `generators/objectlist.js:208`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Access {
    #[default]
    Ro,
    Rw,
    Wo,
    RwPre,
}

impl Access {
    pub fn from_str(s: &str) -> Option<Access> {
        match s {
            "RO" => Some(Access::Ro),
            "RW" => Some(Access::Rw),
            "WO" => Some(Access::Wo),
            "RWpre" => Some(Access::RwPre),
            _ => None,
        }
    }

    /// Note: lowercase `pre` in `RWpre`, matching the JS reference verbatim.
    pub fn macro_suffix(&self) -> &'static str {
        match self {
            Access::Ro => "RO",
            Access::Rw => "RW",
            Access::Wo => "WO",
            Access::RwPre => "RWpre",
        }
    }
}

/// PDO mapping direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdoDir {
    None,
    Tx,
    Rx,
}

/// Supported EtherCAT Slave Controller chips. Source: `SupportedESC` in
/// `constants.js:159-166`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esc {
    Ax58100,
    Et1100,
    Lan9252,
    Lan9253Beckhoff,
    Lan9253Direct,
    Lan9253Indirect,
}

impl Esc {
    /// Parses the reference display strings (`SupportedESC` values in `constants.js:159-166`).
    pub fn from_str(s: &str) -> Option<Esc> {
        match s {
            "AX58100" => Some(Esc::Ax58100),
            "ET1100" => Some(Esc::Et1100),
            "LAN9252" => Some(Esc::Lan9252),
            "LAN9253 Beckhoff" => Some(Esc::Lan9253Beckhoff),
            "LAN9253 Direct" => Some(Esc::Lan9253Direct),
            "LAN9253 Indirect" => Some(Esc::Lan9253Indirect),
            _ => None,
        }
    }

    /// EEPROM word 0 byte 0: PDI control. Source: `EEPROM.js:29-56`.
    /// Default/AX58100/LAN9253 Beckhoff leave the 0x05 default untouched;
    /// LAN9252 and LAN9253 Indirect set 0x80; LAN9253 Direct sets 0x82.
    pub fn pdi_control(&self) -> u8 {
        match self {
            Esc::Ax58100 => 0x05,
            Esc::Et1100 => 0x05,
            Esc::Lan9252 => 0x80,
            Esc::Lan9253Beckhoff => 0x05,
            Esc::Lan9253Direct => 0x82,
            Esc::Lan9253Indirect => 0x80,
        }
    }

    /// EEPROM word address 5: reserved config word. Source: `EEPROM.js:29-56`.
    pub fn reserved_0x05(&self) -> u16 {
        match self {
            Esc::Ax58100 => 0x001A,
            Esc::Et1100 => 0x0000,
            Esc::Lan9252 => 0x0000,
            Esc::Lan9253Beckhoff => 0xC040,
            Esc::Lan9253Direct => 0xC040,
            Esc::Lan9253Indirect => 0xC040,
        }
    }

    /// Whether this ESC uses reserved EEPROM bytes for configuration.
    /// Source: `configOnReservedBytes` in `constants.js:169-174`.
    pub fn config_on_reserved_bytes(&self) -> bool {
        matches!(
            self,
            Esc::Ax58100 | Esc::Lan9253Beckhoff | Esc::Lan9253Direct | Esc::Lan9253Indirect
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn esi_table_matches_reference() {
        assert_eq!(Dtype::Unsigned32.esi(), EsiType { iec_name: "UDINT", bitsize: 32, ctype: "uint32_t" });
        assert_eq!(Dtype::Integer64.esi().bitsize, 64);              // numeric, not "64"
        assert_eq!(Dtype::Real32.esi(), EsiType { iec_name: "REAL", bitsize: 32, ctype: "float" });
        assert_eq!(Dtype::VisibleString.esi(), EsiType { iec_name: "STRING", bitsize: 8, ctype: "char" });
        assert_eq!(Dtype::Boolean.esi(), EsiType { iec_name: "BOOL", bitsize: 1, ctype: "uint8_t" });
    }
    #[test]
    fn macro_and_ident_roundtrip() {
        assert_eq!(Dtype::Unsigned32.macro_suffix(), "UNSIGNED32");
        assert_eq!(Dtype::from_ident("VISIBLE_STRING"), Some(Dtype::VisibleString));
        assert_eq!(Access::RwPre.macro_suffix(), "RWpre");
    }
    #[test]
    fn esc_config_bytes() {
        assert!(Esc::Ax58100.config_on_reserved_bytes());
        assert!(Esc::Lan9253Direct.config_on_reserved_bytes());
        assert!(!Esc::Et1100.config_on_reserved_bytes());
        assert_eq!(Esc::from_str("LAN9253 Beckhoff"), Some(Esc::Lan9253Beckhoff));
    }
}
