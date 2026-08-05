//! `objectlist.c` generator — port of
//! `.cache/EEPROM_generator/src/generators/objectlist.js`
//! (`objectlist_generator`, lines 66-237).
//!
//! Emits three blocks per OD index: the `acName*` string constants, the
//! `SDO<index>[]` `_objd` row array, and the `SDOobjects[]` dictionary row
//! (terminated by the `{0xffff, 0xff, 0xff, 0xff, NULL, NULL}` sentinel).
//! The `data` links (`&Obj.foo`, quoted string literals) are already baked
//! into `Od` by `od_build::link_utypes` (the Rust home of the JS
//! `objectlist_link_utypes`, `objectlist.js:29-64`), so this module only
//! formats what's already there.
//!
//! **BUG-1 FIX (intentional divergence):** the JS reference's
//! `objectlist_getItemValue` (`objectlist.js:183-196`) hex-encodes the
//! *placeholder* string `'0'` for REAL32 — `float32ToHex(value)` reads the
//! local `let value = '0'` shadowing variable, never `objd.value` — so
//! every REAL32 default is emitted as `0x00000000` regardless of its actual
//! value. This port encodes the real value (`float32_to_hex`, below).

use serde_json::Value;

use crate::model::{Config, Objd};
use crate::od_build::Od;
use crate::types::{Access, Dtype, PdoDir};

pub fn generate(_config: &Config, od: &Od) -> String {
    let mut out =
        String::from("#include \"esc_coe.h\"\n#include \"utypes.h\"\n#include <stddef.h>\n\n");

    for (&idx, objd) in od {
        out.push_str(&variable_name_block(idx, objd));
    }
    out.push('\n');

    for (&idx, objd) in od {
        out.push_str(&sdo_declaration_block(idx, objd));
    }

    out.push_str("\n\nconst _objectlist SDOobjects[] =\n{");
    for (&idx, objd) in od {
        out.push_str(&dictionary_declaration(idx, objd));
    }
    out.push_str("\n  {0xffff, 0xff, 0xff, 0xff, NULL, NULL}\n};\n");

    out
}

/// `objectlist_variableName`, `objectlist.js:93-110`.
fn variable_name_block(idx: u16, objd: &Objd) -> String {
    let index = format!("{idx:X}");
    let mut s = format!("\nstatic const char acName{index}[] = \"{}\";", objd.name());
    match objd {
        Objd::Var { .. } => {}
        Objd::Array { items, .. } | Objd::Record { items, .. } => {
            for (subindex, item) in items.iter().enumerate() {
                let subi = subindex_padded(subindex as u16);
                s.push_str(&format!(
                    "\nstatic const char acName{index}_{subi}[] = \"{}\";",
                    item.name
                ));
            }
        }
    }
    s
}

/// `objectlist_SdoObjectDeclaration`, `objectlist.js:112-154`.
fn sdo_declaration_block(idx: u16, objd: &Objd) -> String {
    let index = format!("{idx:X}");
    let mut s = format!("\nconst _objd SDO{index}[] =\n{{");

    match objd {
        Objd::Var { dtype, size, value, access, data, .. } => {
            let dtype = dtype.expect("VAR objd without dtype");
            let bitsize = var_row_bitsize(dtype, *size, value.as_ref());
            let val = item_value(dtype, value.as_ref());
            let flags = atype_flags(*access, objd.pdo_dir());
            let data = objd_data(dtype, value.as_ref(), data.as_deref());
            s.push_str(&format!(
                "\n  {{0x0, DTYPE_{}, {bitsize}, {flags}, acName{index}, {val}, {data}}},",
                dtype.macro_suffix()
            ));
        }
        Objd::Array { dtype, items, access, .. } => {
            let dtype = dtype.expect("ARRAY objd without dtype");
            let maxsubindex = items.len() - 1;
            s.push_str(&format!(
                "\n  {{0x00, DTYPE_UNSIGNED8, 8, ATYPE_RO, acName{index}_00, {maxsubindex}, NULL}},"
            ));
            let bitsize = dtype.esi().bitsize; // TODO (reference too): array of strings not sized
            let flags = atype_flags(*access, objd.pdo_dir());
            for (i, item) in items.iter().enumerate().skip(1) {
                let subi = subindex_padded(i as u16);
                let val = item_value(dtype, item.value.as_ref());
                let data = item.data.as_deref().filter(|d| !d.is_empty()).unwrap_or("NULL");
                s.push_str(&format!(
                    "\n  {{0x{subi}, DTYPE_{}, {bitsize}, {flags}, acName{index}_{subi}, {val}, {data}}},",
                    dtype.macro_suffix()
                ));
            }
        }
        Objd::Record { items, .. } => {
            let maxsubindex = items.len() - 1;
            s.push_str(&format!(
                "\n  {{0x00, DTYPE_UNSIGNED8, 8, ATYPE_RO, acName{index}_00, {maxsubindex}, NULL}},"
            ));
            for (i, item) in items.iter().enumerate().skip(1) {
                let subi = subindex_padded(i as u16);
                let subdtype = item.dtype.expect("RECORD subitem without dtype");
                let bitsize = subdtype.esi().bitsize;
                let val = item_value(subdtype, item.value.as_ref());
                // RECORD subitems carry no `pdo_mappings` (only top-level
                // objd does), so their flags never gain a PDO OR-term
                // (objectlist_objdFlags(subitem), objectlist.js:211-223).
                let flags = atype_flags(item.access, PdoDir::None);
                let data = item.data.as_deref().filter(|d| !d.is_empty()).unwrap_or("NULL");
                s.push_str(&format!(
                    "\n  {{0x{subi}, DTYPE_{}, {bitsize}, {flags}, acName{index}_{subi}, {val}, {data}}},",
                    subdtype.macro_suffix()
                ));
            }
        }
    }

    s.push_str("\n};");
    s
}

/// `objectlist_DictionaryDeclaration`, `objectlist.js:156-173`. `pad1` isn't
/// modeled (never set by anything in `od_build`), so it's always `0`.
fn dictionary_declaration(idx: u16, objd: &Objd) -> String {
    let index = format!("{idx:X}");
    let (otype, maxsubindex) = match objd {
        Objd::Var { .. } => ("VAR", 0),
        Objd::Array { items, .. } => ("ARRAY", items.len() - 1),
        Objd::Record { items, .. } => ("RECORD", items.len() - 1),
    };
    format!("\n  {{0x{index}, OTYPE_{otype}, {maxsubindex}, 0, acName{index}, SDO{index}}},")
}

/// `get_objdBitsize`, `objectlist.js:16-22` (VAR row only — ARRAY/RECORD
/// rows use the dtype's plain bitsize, `objectlist.js:123,138`).
fn var_row_bitsize(dtype: Dtype, size: Option<u16>, value: Option<&Value>) -> u16 {
    let bitsize = dtype.esi().bitsize;
    if dtype != Dtype::VisibleString {
        return bitsize;
    }
    match size {
        Some(s) if s != 0 => bitsize * s,
        _ => value.and_then(Value::as_str).map(|s| s.len() as u16).unwrap_or(0),
    }
}

/// `objectlist_getItemValue`, `objectlist.js:183-196`.
fn item_value(dtype: Dtype, value: Option<&Value>) -> String {
    let Some(v) = value else { return "0".to_string() };
    match dtype {
        Dtype::Real32 => {
            let f: f32 = value_to_string(v).trim().parse().unwrap_or(0.0);
            format!("0x{}", float32_to_hex(f))
        }
        // VISIBLE_STRING: deliberately left as the placeholder '0' — the
        // actual string travels in the `data` column instead (see
        // `objd_data` / objectlist.js:225-236, VISIBLE_STRING branch of
        // `objectlist_getItemValue` returns the same placeholder). Not a
        // bug, spec-pinned.
        Dtype::VisibleString => "0".to_string(),
        // REAL64 and everything else: pass through verbatim (default
        // branch, objectlist.js:192) — not a bug either.
        _ => value_to_string(v),
    }
}

/// `objectlist_objdData`, `objectlist.js:225-236` — VAR row only. ARRAY/
/// RECORD subitem rows use `subitem.data` directly (objectlist.js:128,141),
/// no quoting fallback.
fn objd_data(dtype: Dtype, value: Option<&Value>, data: Option<&str>) -> String {
    if let Some(d) = data
        && !d.is_empty()
    {
        return d.to_string();
    }
    if dtype == Dtype::VisibleString
        && let Some(s) = value.map(value_to_string).filter(|s| !s.is_empty())
    {
        return format!("\"{s}\"");
    }
    "NULL".to_string()
}

/// `objectlist_objdFlags`, `objectlist.js:211-223`.
fn atype_flags(access: Option<Access>, dir: PdoDir) -> String {
    let mut flags = format!("ATYPE_{}", access.unwrap_or_default().macro_suffix());
    match dir {
        PdoDir::Tx => flags.push_str(" | ATYPE_TXPDO"),
        PdoDir::Rx => flags.push_str(" | ATYPE_RXPDO"),
        PdoDir::None => {}
    }
    flags
}

/// `subindex_padded`, `objectlist.js:198-204`. REFERENCE QUIRK: zero-padded
/// to 2 **decimal** digits, then emitted after a literal `0x` at call
/// sites — subindex 10 becomes the text `0x10`, meaning decimal 10, not
/// hex. `{:02}` (decimal, width 2) reproduces this for the full practical
/// subindex range (JS never left-pads past 2 digits either: `${subindex}`
/// once `subindex > 9`).
fn subindex_padded(subindex: u16) -> String {
    format!("{subindex:02}")
}

/// IEEE-754 single-precision bit pattern as uppercase hex, no `0x` prefix
/// (callers add it). Used only by the bug-1 fix above.
fn float32_to_hex(v: f32) -> String {
    format!("{:08X}", v.to_bits())
}

/// `${objd.value}` JS template-literal stringification: strings pass
/// through raw (no added quoting), everything else uses its plain string
/// form (numbers print as plain decimal, matching `Number.prototype.toString`).
fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
