//! Object dictionary builder — port of `.cache/EEPROM_generator/src/od.js:91-291`
//! (`buildObjectDictionary` + its helpers) and the `data`-link generation in
//! `.cache/EEPROM_generator/src/generators/objectlist.js:29-64`
//! (`objectlist_link_utypes`).
//!
//! Merges, in order: mandatory objects (`getMandatoryObjects` +
//! `populateMandatoryObjectValues`), SDO items (`addSDOitems`), then TX
//! then RX PDO items (`addTXPDOitems`/`addRXPDOitems` via
//! `addPdoObjectsSection`), which also synthesize the SM-assignment arrays
//! (`1C13`/`1C12`) and per-object PDO mapping records (`0x1Axx`/`0x14xx`).

use std::collections::BTreeMap;

use serde_json::json;

use crate::model::{Config, Objd, OdMap, OdSections, SubItem};
use crate::names::{parse_u32, variable_name};
use crate::types::Dtype;
use crate::GenError;

/// Flat, merged object dictionary: EtherCAT CoE index -> object.
pub type Od = BTreeMap<u16, Objd>;

/// Builds the complete object dictionary from the UI form (`config`) and the
/// three edited OD sections (`od`). Mirrors `buildObjectDictionary`,
/// `od.js:282-291`, plus the spec's validation set (`GenError::Od`).
pub fn build_object_dictionary(config: &Config, od: &OdSections) -> Result<Od, GenError> {
    validate(od)?;

    let mut result = mandatory_objects();
    populate_mandatory_object_values(config, &mut result)?;

    // populate custom objects
    add_sdo_items(&od.sdo, &mut result);

    let mut boolean_padding_count: u32 = 0;
    // TX PDO -> SM3Offset / 1C13 ("Regardless of value set, SDK was
    // generating RXPDO mappings as SDO1400" — od.js:114-119,130-139).
    let sm3_offset = parse_u32("SM3Offset", &config.sm3_offset)?;
    add_pdo_objects_section(
        &mut result,
        &od.txpdo,
        "txpdo",
        0x1C13,
        sm3_offset,
        &mut boolean_padding_count,
    );
    // RX PDO -> SM2Offset / 1C12
    let sm2_offset = parse_u32("SM2Offset", &config.sm2_offset)?;
    add_pdo_objects_section(
        &mut result,
        &od.rxpdo,
        "rxpdo",
        0x1C12,
        sm2_offset,
        &mut boolean_padding_count,
    );

    Ok(result)
}

// ####################### Validation set (od_build's own gate) ####################### //
//
// Not present as a single function in the JS reference — these are the
// UI-time checks from `ui.js` (`odModalSaveChanges`), applied up front to
// the whole (used-index) OD so a malformed project fails loudly instead of
// producing a broken generator output.

fn validate(od: &OdSections) -> Result<(), GenError> {
    let sections: [&OdMap; 3] = [&od.sdo, &od.txpdo, &od.rxpdo];

    // 1. object-name uniqueness across sections (findObjectIndexByName,
    //    ui.js:434 / od.js:72-86). Only over `getUsedIndexes`-visible
    //    entries — a section may carry dead/malformed keys the reference
    //    tool itself would never surface (see `used_indexes`).
    let mut seen: std::collections::HashMap<&str, u16> = std::collections::HashMap::new();
    for section in sections {
        for idx in used_indexes(section) {
            let objd = &section[&hex_key(idx)];
            if let Some(&prev) = seen.get(objd.name()) {
                return Err(GenError::Od(format!(
                    "object {:?} duplicates 0x{prev:X}: already used by object 0x{idx:X}",
                    objd.name()
                )));
            }
            seen.insert(objd.name(), idx);
        }
    }

    // 2/3: dtype + VISIBLE_STRING size checks (ui.js:441-462, VAR case only
    // — ARRAY/RECORD have no equivalent check in the reference).
    for section in sections {
        for idx in used_indexes(section) {
            let objd = &section[&hex_key(idx)];
            let Objd::Var { dtype: Some(dtype), size, value, pdo_mappings, name, .. } = objd else {
                continue;
            };

            // 2. PDO-mapped dtype must be in dtypes_PDO_allowed (constants.js:93,
            //    ui.js:445). Of our 12 Dtype variants, only VISIBLE_STRING is
            //    excluded from that set.
            if !pdo_mappings.is_empty() && !dtype_pdo_allowed(*dtype) {
                return Err(GenError::Od(format!(
                    "data type {} cannot be used in PDO-mapped object {name:?} (0x{idx:X})",
                    dtype.macro_suffix()
                )));
            }

            // 3. VISIBLE_STRING size >= initial value length (getMinimumStringLength,
            //    ui.js:249-251,404-408).
            if *dtype == Dtype::VisibleString {
                let value_len = value.as_ref().and_then(|v| v.as_str()).map(str::len).unwrap_or(0);
                let size = size.unwrap_or(0) as usize;
                if size < value_len {
                    return Err(GenError::Od(format!(
                        "object {name:?} (0x{idx:X}): VISIBLE_STRING size {size} is smaller than initial value length {value_len}"
                    )));
                }
            }
        }
    }

    Ok(())
}

/// `dtypes_PDO_allowed`, `constants.js:93-116`. That set lists bit-types
/// (`BIT1..8`, `BITARRn`) our `Dtype` doesn't model (task scope: 12 CoE
/// dtypes, `types.rs`) plus every one of our 12 dtypes except
/// VISIBLE_STRING — so membership reduces to "not VISIBLE_STRING".
fn dtype_pdo_allowed(d: Dtype) -> bool {
    d != Dtype::VisibleString
}

// ####################### Mandatory objects ####################### //

/// `getMandatoryObjects`, `constants.js:129-151`.
fn mandatory_objects() -> Od {
    let mut od = Od::new();

    let (_, mut device_type) = Objd::var(0x1000, Dtype::Unsigned32, "Device Type");
    device_type.set_value(json!(0x1389u32));
    od.insert(0x1000, device_type);

    let (_, mut device_name) = Objd::var(0x1008, Dtype::VisibleString, "Device Name");
    device_name.set_data(""); // literal `data: ''` in the reference
    od.insert(0x1008, device_name);

    let (_, mut hw_version) = Objd::var(0x1009, Dtype::VisibleString, "Hardware Version");
    hw_version.set_data("");
    od.insert(0x1009, hw_version);

    let (_, mut sw_version) = Objd::var(0x100A, Dtype::VisibleString, "Software Version");
    sw_version.set_data("");
    od.insert(0x100A, sw_version);

    let identity_items = vec![
        SubItem { name: "Max SubIndex".into(), dtype: None, value: None, access: None, data: None },
        SubItem {
            name: "Vendor ID".into(),
            dtype: Some(Dtype::Unsigned32),
            value: Some(json!(600)),
            access: None,
            data: None,
        },
        SubItem { name: "Product Code".into(), dtype: Some(Dtype::Unsigned32), value: None, access: None, data: None },
        SubItem { name: "Revision Number".into(), dtype: Some(Dtype::Unsigned32), value: None, access: None, data: None },
        SubItem {
            name: "Serial Number".into(),
            dtype: Some(Dtype::Unsigned32),
            value: None,
            access: None,
            data: Some("&Obj.serial".into()),
        },
    ];
    let (_, identity) = Objd::record(0x1018, "Identity Object", identity_items);
    od.insert(0x1018, identity);

    let sm_comm_items = vec![
        SubItem { name: "Max SubIndex".into(), dtype: None, value: None, access: None, data: None },
        SubItem { name: "Communications Type SM0".into(), dtype: None, value: Some(json!(1)), access: None, data: None },
        SubItem { name: "Communications Type SM1".into(), dtype: None, value: Some(json!(2)), access: None, data: None },
        SubItem { name: "Communications Type SM2".into(), dtype: None, value: Some(json!(3)), access: None, data: None },
        SubItem { name: "Communications Type SM3".into(), dtype: None, value: Some(json!(4)), access: None, data: None },
    ];
    let (_, sm_comm) = Objd::array(0x1C00, Dtype::Unsigned8, "Sync Manager Communication Type", sm_comm_items);
    od.insert(0x1C00, sm_comm);

    od
}

/// `populateMandatoryObjectValues`, `od.js:267-280`.
fn populate_mandatory_object_values(config: &Config, od: &mut Od) -> Result<(), GenError> {
    let device_name = &config.text_device_name;
    let dev_obj = od.get_mut(&0x1008).expect("mandatory 0x1008");
    dev_obj.set_value(json!(device_name));
    dev_obj.set_size(device_name.len() as u16);

    let hw_version = &config.hw_version;
    let hw_obj = od.get_mut(&0x1009).expect("mandatory 0x1009");
    hw_obj.set_value(json!(hw_version));
    hw_obj.set_size(hw_version.len() as u16);

    let sw_version = &config.sw_version;
    let sw_obj = od.get_mut(&0x100A).expect("mandatory 0x100A");
    sw_obj.set_value(json!(sw_version));
    sw_obj.set_size(sw_version.len() as u16);

    let vendor_id = parse_u32("VendorID", &config.vendor_id)?;
    let product_code = parse_u32("ProductCode", &config.product_code)?;
    let revision_number = parse_u32("RevisionNumber", &config.revision_number)?;
    let serial_number = parse_u32("SerialNumber", &config.serial_number)?;

    let identity_items = od.get_mut(&0x1018).expect("mandatory 0x1018").items_mut();
    identity_items[1].value = Some(json!(vendor_id));
    identity_items[2].value = Some(json!(product_code));
    identity_items[3].value = Some(json!(revision_number));
    identity_items[4].value = Some(json!(serial_number));

    Ok(())
}

// ####################### SDO items ####################### //

/// `addSDOitems`, `od.js:91-102`.
fn add_sdo_items(sdo: &OdMap, od: &mut Od) {
    for idx in used_indexes(sdo) {
        let mut objd = sdo[&hex_key(idx)].clone();
        objd.set_is_sdo_item(true);
        link_utypes(&mut objd);
        od.insert(idx, objd);
    }
}

// ####################### PDO items ####################### //

/// `addPdoObjectsSection`, `od.js:161-265`. `sm_index` is `0x1C13` (TX) or
/// `0x1C12` (RX); `sm_offset` is `SM3Offset`/`SM2Offset` parsed from the
/// form (the first PDO-mapped object lands there, then one index per object).
fn add_pdo_objects_section(
    od: &mut Od,
    section: &OdMap,
    pdo_name: &str,
    sm_index: u16,
    sm_offset: u32,
    boolean_padding_count: &mut u32,
) {
    let indexes = used_indexes(section);
    if indexes.is_empty() {
        return;
    }

    // ensurePDOAssignmentExists, od.js:243-252
    od.entry(sm_index).or_insert_with(|| {
        let sm_char = format!("{sm_index:X}").chars().last().unwrap();
        let (_, assignment) = Objd::array(
            sm_index,
            Dtype::Unsigned16,
            format!("Sync Manager {sm_char} PDO Assignment"),
            vec![SubItem { name: "Max SubIndex".into(), dtype: None, value: None, access: None, data: None }],
        );
        assignment
    });

    for (current_sm_offset, idx) in (sm_offset..).zip(indexes) {
        let mut objd = section[&hex_key(idx)].clone();
        let current_offset = current_sm_offset as u16;

        objd.add_pdo_mapping(pdo_name);
        link_utypes(&mut objd);

        let mut mapping_items =
            vec![SubItem { name: "Max SubIndex".into(), dtype: None, value: None, access: None, data: None }];

        match &objd {
            Objd::Var { dtype, .. } => {
                let dtype = dtype.expect("VAR objd without dtype");
                let bitsize = var_bitsize(dtype, objd.size());
                mapping_items.push(mapping_subitem(objd.name(), idx, 0, bitsize));
                if dtype == Dtype::Boolean {
                    *boolean_padding_count += 1;
                    mapping_items.push(boolean_padding_subitem(*boolean_padding_count));
                }
            }
            Objd::Array { dtype, items, .. } => {
                let dtype = dtype.expect("ARRAY objd without dtype");
                let bitsize = var_bitsize(dtype, objd.size());
                for (i, subitem) in items.iter().skip(1).enumerate() {
                    let subindex = i as u16 + 1;
                    mapping_items.push(mapping_subitem(&subitem.name, idx, subindex, bitsize));
                    // TODO handle padding on array of booleans (od.js:195, not
                    // implemented in the reference either).
                }
            }
            Objd::Record { items, .. } => {
                for (i, subitem) in items.iter().skip(1).enumerate() {
                    let subindex = i as u16 + 1;
                    // RECORD sub-items carry no `size` field (only VAR/ARRAY
                    // top-level objects do) — VISIBLE_STRING sizing on a
                    // record sub-item is unsupported here, same as the
                    // reference's `subitem.size` (undefined -> NaN).
                    let bitsize = subitem.dtype.map(|d| var_bitsize(d, None)).unwrap_or(0);
                    mapping_items.push(mapping_subitem(&subitem.name, idx, subindex, bitsize));
                    if subitem.dtype == Some(Dtype::Boolean) {
                        *boolean_padding_count += 1;
                        mapping_items.push(boolean_padding_subitem(*boolean_padding_count));
                    }
                }
            }
        }

        let (_, mapping_objd) = Objd::record(current_offset, objd.name().to_string(), mapping_items);
        od.insert(current_offset, mapping_objd);

        od.get_mut(&sm_index)
            .expect("SM assignment array just ensured")
            .items_mut()
            .push(SubItem {
                name: "PDO Mapping".into(),
                dtype: None,
                value: Some(json!(format!("0x{current_offset:04X}"))),
                access: None,
                data: None,
            });

        od.insert(idx, objd);
    }
}

/// `getPdoMappingValue`, `od.js:254-264`: builds one PDO-mapping sub-item
/// (`{name, dtype: UNSIGNED32, value}`).
fn mapping_subitem(name: &str, index: u16, subindex: u16, bitsize: u16) -> SubItem {
    SubItem {
        name: name.to_string(),
        dtype: Some(Dtype::Unsigned32),
        value: Some(json!(pdo_mapping_value(index, subindex, bitsize))),
        access: None,
        data: None,
    }
}

/// `getPdoMappingValue`'s `0x${index}${toByte(subindex)}${toByte(bitsize)}`.
/// `index` is always the 4-uppercase-hex-digit canonical form (indexes in
/// `[0x1000, 0xFFFF]` never need padding); `toByte` lowercases (JS
/// `Number.toString(16)` default), so this mixes case on purpose, matching
/// the reference's actual output byte-for-byte.
fn pdo_mapping_value(index: u16, subindex: u16, bitsize: u16) -> String {
    format!("0x{index:04X}{subindex:02x}{bitsize:02x}")
}

/// `varBitsize`, `od.js:146-153`. `pub(crate)`: reused by `generators::esi`
/// (`esi_xml.js`'s `varBitsize` is the very same shared helper, not a
/// re-derivation).
pub(crate) fn var_bitsize(dtype: Dtype, size: Option<u16>) -> u16 {
    let mut bitsize = dtype.esi().bitsize;
    if dtype == Dtype::VisibleString {
        bitsize *= size.unwrap_or(0);
    }
    bitsize
}

/// `addBooleanPadding`, `od.js:227-229`. The item's value hardcodes the
/// global `booleanPaddingBitsize` constant (=7) regardless of `n` — only the
/// name is numbered.
fn boolean_padding_subitem(n: u32) -> SubItem {
    const BOOLEAN_PADDING_BITSIZE: u32 = 7;
    SubItem {
        name: format!("Padding {n}"),
        dtype: Some(Dtype::Unsigned32),
        value: Some(json!(format!("0x{BOOLEAN_PADDING_BITSIZE:08X}"))),
        access: None,
        data: None,
    }
}

// ####################### shared helpers ####################### //

/// `objectlist_link_utypes`, `generators/objectlist.js:29-64`. Adds
/// `objd.data`/`subitem.data` links to the OD struct declared in `utypes.h`.
fn link_utypes(objd: &mut Objd) {
    match objd {
        Objd::Var { name, data, .. } => {
            *data = Some(format!("&Obj.{}", variable_name(name)));
        }
        Objd::Array { name, items, .. } => {
            let base = variable_name(name);
            for (i, item) in items.iter_mut().skip(1).enumerate() {
                item.data = Some(format!("&Obj.{base}[{i}]"));
            }
        }
        Objd::Record { name, items, .. } => {
            let base = variable_name(name);
            for item in items.iter_mut().skip(1) {
                let sub = variable_name(&item.name);
                item.data = Some(format!("&Obj.{base}.{sub}"));
            }
        }
    }
}

/// `getUsedIndexes`, `od.js:300-313`: only string keys that are the exact
/// canonical uppercase-hex form of some index in `[0x1000, 0xFFFF]` are
/// "used" — anything else (malformed/legacy keys, e.g. a stray `"A"`) is
/// silently invisible to the reference tool, and must be to us too.
fn used_indexes(section: &OdMap) -> Vec<u16> {
    let mut v: Vec<u16> = section
        .keys()
        .filter_map(|k| {
            let i = u16::from_str_radix(k, 16).ok()?;
            (0x1000..=0xFFFF).contains(&i).then_some(())?;
            (hex_key(i) == *k).then_some(i)
        })
        .collect();
    v.sort_unstable();
    v
}

fn hex_key(index: u16) -> String {
    format!("{index:X}")
}
