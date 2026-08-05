//! `device.xml` (ESI XML) generator — port of
//! `.cache/EEPROM_generator/src/generators/esi_xml.js` (`esi_generator`).
//!
//! **BUG-4 FIX (intentional divergence):** `Physics` concatenates ALL FOUR
//! `PortNPhysical` values (`P0+P1+P2+P3`), matching `getPhysicalPort`
//! (`EEPROM.js:281-302`, which the EEPROM binary's General category uses).
//! The JS reference (`esi_xml.js:26`) has an operator-precedence bug:
//! `` `${P0 + P1 + P2 || +P3}` `` — `||` binds looser than `+`, so the
//! expression parses as `(P0+P1+P2) || (+P3)`; since `P0+P1+P2` is normally
//! a non-empty (truthy) string, `Port3Physical` is silently dropped from the
//! `Physics` attribute. This port always emits all four ports.
//!
//! **XML escaping (also a divergence — the JS reference never escapes):**
//! every user-supplied text/attribute value goes through `xml_escape`.
//!
//! **Multiple PDO mappings on one object** — the JS reference `alert()`s
//! and silently uses only the first mapping (`getPdoMappingFlags`,
//! `esi_xml.js:286-297`); this port returns `GenError::Od` instead.

use indexmap::IndexMap;
use serde_json::Value;

use crate::generators::eeprom;
use crate::model::{Config, Objd, SubItem, SyncMode};
use crate::names::parse_u32;
use crate::od_build::{var_bitsize, Od};
use crate::types::{Access, Dtype};
use crate::GenError;

pub fn generate(config: &Config, od: &Od, dc: &[SyncMode]) -> Result<String, GenError> {
    let vendor_id = parse_u32("VendorID", &config.vendor_id)?;
    let product_code = parse_u32("ProductCode", &config.product_code)?;
    let revision_number = parse_u32("RevisionNumber", &config.revision_number)?;
    let mailbox_size = parse_u32("MailboxSize", &config.mailbox_size)?;
    let rx_mailbox_offset = parse_u32("RxMailboxOffset", &config.rx_mailbox_offset)?;
    let tx_mailbox_offset = parse_u32("TxMailboxOffset", &config.tx_mailbox_offset)?;
    let sm2_offset = parse_u32("SM2Offset", &config.sm2_offset)?;
    let sm3_offset = parse_u32("SM3Offset", &config.sm3_offset)?;
    let eeprom_size = parse_u32("EEPROMsize", &config.eeprom_size)?;

    // header + Vendor
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<EtherCATInfo>\n  <Vendor>\n    <Id>{vendor_id}</Id>\n"
    );
    out.push_str(&format!(
        "    <Name LcId=\"1033\">{}</Name>\n  </Vendor>\n  <Descriptions>\n",
        xml_escape(&config.vendor_name)
    ));

    // Groups
    out.push_str(&format!(
        "    <Groups>\n      <Group>\n        <Type>{}</Type>\n        <Name LcId=\"1033\">{}</Name>\n      </Group>\n    </Groups>\n    <Devices>\n",
        xml_escape(&config.text_group_type),
        xml_escape(&config.text_group_name5)
    ));

    // Device open + Physics (bug-4 fix: all four ports) + Type + Name + GroupType
    let physics = format!(
        "{}{}{}{}",
        xml_escape(&config.port0_physical),
        xml_escape(&config.port1_physical),
        xml_escape(&config.port2_physical),
        xml_escape(&config.port3_physical),
    );
    out.push_str(&format!(
        "      <Device Physics=\"{physics}\">\n        <Type ProductCode=\"#x{product_code:x}\" RevisionNo=\"#x{revision_number:x}\">{}</Type>\n",
        xml_escape(&config.text_device_type)
    ));
    out.push_str(&format!(
        "        <Name LcId=\"1033\">{}</Name>\n",
        xml_escape(&config.text_device_name)
    ));
    out.push_str(&format!(
        "        <GroupType>{}</GroupType>\n",
        xml_escape(&config.text_group_type)
    ));

    // Profile + DataTypes
    out.push_str(&format!(
        "        <Profile>\n          <ProfileNo>{}</ProfileNo>\n          <AddInfo>0</AddInfo>\n          <Dictionary>\n            <DataTypes>",
        xml_escape(&config.profile_no)
    ));

    let mut variable_types: IndexMap<String, u16> = IndexMap::new();
    for (&idx, objd) in od {
        out.push_str(&add_object_dictionary_data_type(idx, objd, &mut variable_types)?);
    }
    for (name, bitsize) in &variable_types {
        out.push_str(&format!(
            "\n              <DataType>\n                <Name>{name}</Name>\n                <BitSize>{bitsize}</BitSize>\n              </DataType>"
        ));
    }
    out.push_str("\n            </DataTypes>\n            <Objects>");

    // Objects
    for (&idx, objd) in od {
        out.push_str(&add_dictionary_object(idx, objd)?);
    }

    let is_rxpdo = od.values().any(|o| o.pdo_mappings().iter().any(|m| m == "rxpdo"));
    let is_txpdo = od.values().any(|o| o.pdo_mappings().iter().any(|m| m == "txpdo"));

    out.push_str(
        "\n            </Objects>\n          </Dictionary>\n        </Profile>\n        <Fmmu>Outputs</Fmmu>\n        <Fmmu>Inputs</Fmmu>\n        <Fmmu>MBoxState</Fmmu>\n",
    );
    out.push_str(&format!(
        "        <Sm DefaultSize=\"{mailbox_size}\" StartAddress=\"#x{rx_mailbox_offset:X}\" ControlByte=\"#x26\" Enable=\"1\">MBoxOut</Sm>\n"
    ));
    out.push_str(&format!(
        "        <Sm DefaultSize=\"{mailbox_size}\" StartAddress=\"#x{tx_mailbox_offset:X}\" ControlByte=\"#x22\" Enable=\"1\">MBoxIn</Sm>\n"
    ));
    out.push_str(&format!(
        "        <Sm StartAddress=\"#x{sm2_offset:X}\" ControlByte=\"#x24\" Enable=\"{}\">Outputs</Sm>\n",
        if is_rxpdo { 1 } else { 0 }
    ));
    out.push_str(&format!(
        "        <Sm StartAddress=\"#x{sm3_offset:X}\" ControlByte=\"#x20\" Enable=\"{}\">Inputs</Sm>\n",
        if is_txpdo { 1 } else { 0 }
    ));

    if is_rxpdo {
        let mut mem_offset = sm2_offset;
        for (&idx, objd) in od {
            if objd.pdo_mappings().iter().any(|m| m == "rxpdo") {
                out.push_str(&add_esi_device_pdo(objd, idx, "rxpdo", mem_offset)?);
                mem_offset += 1;
            }
        }
    }
    if is_txpdo {
        let mut mem_offset = sm3_offset;
        for (&idx, objd) in od {
            if objd.pdo_mappings().iter().any(|m| m == "txpdo") {
                out.push_str(&add_esi_device_pdo(objd, idx, "txpdo", mem_offset)?);
                mem_offset += 1;
            }
        }
    }

    // Mailbox DLL
    out.push_str(&format!(
        "        <Mailbox DataLinkLayer=\"true\">\n          <CoE {}/>\n",
        coe_mailbox_attrs(config)
    ));
    if config.details_enable_use_foe {
        out.push_str("          <FoE/>\n");
    }
    out.push_str("        </Mailbox>\n");

    // DCs
    out.push_str(&dc_section(dc));

    // Eeprom
    let config_data = eeprom::config_data_string(config)?;
    out.push_str(&format!(
        "        <Eeprom>\n          <ByteSize>{eeprom_size}</ByteSize>\n          <ConfigData>{config_data}</ConfigData>\n        </Eeprom>\n"
    ));

    // Close
    out.push_str("      </Device>\n    </Devices>\n  </Descriptions>\n</EtherCATInfo>");

    Ok(out)
}

/// `xml_escape`: `&`, `<`, `>`, `"`, `'`. Not present in the JS reference at
/// all — a deliberate divergence (see module docs). Order matters: `&` must
/// be escaped first so the entities introduced below aren't re-escaped.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// ####################### DataTypes section ####################### //

/// `addObjectDictionaryDataType`, `esi_xml.js:49-122`. VAR objects only
/// register a variable type (returned text is empty — the `<DataType>`
/// block for the shared variable type is emitted later, after every index
/// has been processed: see the `variable_types` loop in `generate`). ARRAY
/// and RECORD objects each get one dedicated `DT<index>` custom type.
fn add_object_dictionary_data_type(
    idx: u16,
    objd: &Objd,
    variable_types: &mut IndexMap<String, u16>,
) -> Result<String, GenError> {
    match objd {
        Objd::Var { dtype, size, .. } => {
            register_variable_type(dtype.expect("VAR objd without dtype"), *size, variable_types);
            Ok(String::new())
        }
        Objd::Array { dtype, items, pdo_mappings, .. } => {
            let dtype = dtype.expect("ARRAY objd without dtype");
            register_variable_type(dtype, objd.size(), variable_types);

            let esi_type = dtype.esi();
            let dt_name = format!("DT{idx:X}");
            let elements = items.len() as u16 - 1;
            let arr_bitsize = elements * esi_type.bitsize;

            let mut result = String::from("\n              <DataType>");
            result.push_str(&format!(
                "\n                <Name>{dt_name}ARR</Name>\n                <BaseType>{}</BaseType>\n                <BitSize>{arr_bitsize}</BitSize>",
                esi_type.iec_name
            ));
            result.push_str(&format!(
                "\n                <ArrayInfo>\n                  <LBound>1</LBound>\n                  <Elements>{elements}</Elements>\n                </ArrayInfo>"
            ));
            result.push_str("\n              </DataType>");
            result.push_str("\n              <DataType>");

            let bitsize = esi_bitsize(objd);
            result.push_str(&format!(
                "\n                <Name>{dt_name}</Name>\n                <BitSize>{bitsize}</BitSize>"
            ));
            result.push_str(
                "\n                <SubItem>\n                  <SubIdx>0</SubIdx>\n                  <Name>Max SubIndex</Name>\n                  <Type>USINT</Type>\n                  <BitSize>8</BitSize>\n                  <BitOffs>0</BitOffs>\n                  <Flags>\n                    <Access>ro</Access>\n                  </Flags>\n                </SubItem>",
            );

            let pdo_flags = pdo_mapping_flags(pdo_mappings, idx, objd.name())?;
            result.push_str(&format!(
                "\n                <SubItem>\n                  <Name>Elements</Name>\n                  <Type>{dt_name}ARR</Type>\n                  <BitSize>{arr_bitsize}</BitSize>\n                  <BitOffs>16</BitOffs>\n                  <Flags>\n                    <Access>ro</Access>{pdo_flags}\n                  </Flags>\n                </SubItem>"
            ));
            result.push_str("\n              </DataType>");
            Ok(result)
        }
        Objd::Record { items, pdo_mappings, .. } => {
            let dt_name = format!("DT{idx:X}");
            let bitsize = esi_bitsize(objd);

            let mut result = String::from("\n              <DataType>");
            result.push_str(&format!(
                "\n                <Name>{dt_name}</Name>\n                <BitSize>{bitsize}</BitSize>"
            ));
            result.push_str(
                "\n                <SubItem>\n                  <SubIdx>0</SubIdx>\n                  <Name>Max SubIndex</Name>\n                  <Type>USINT</Type>\n                  <BitSize>8</BitSize>\n                  <BitOffs>0</BitOffs>\n                  <Flags>\n                    <Access>ro</Access>\n                  </Flags>\n                </SubItem>",
            );

            // `getSubitemFlags` re-derives the same objd-level PDO flags for
            // every subitem (`esi_xml.js:111-121`); hoisted out of the loop
            // here since the result never changes across subitems.
            let pdo_flags = pdo_mapping_flags(pdo_mappings, idx, objd.name())?;
            let mut bits_offset: u16 = 16;
            for (subindex, subitem) in items.iter().enumerate().skip(1) {
                let subitem_dtype = subitem.dtype.expect("RECORD subitem without dtype");
                register_variable_type(subitem_dtype, None, variable_types);
                // Deliberately NOT `esi_variable_type_name`/`var_bitsize`
                // here: `esi_xml.js:90-91` reads `ESI_DT[subitem.dtype]`
                // directly (no VISIBLE_STRING size handling), unlike the
                // `<Entry>`/registration paths elsewhere in this file.
                let esi = subitem_dtype.esi();
                let subitem_flags = subitem_flags(subitem.access, &pdo_flags);
                result.push_str(&format!(
                    "\n                <SubItem>\n                  <SubIdx>{subindex}</SubIdx>\n                  <Name>{}</Name>\n                  <Type>{}</Type>\n                  <BitSize>{}</BitSize>\n                  <BitOffs>{bits_offset}</BitOffs>\n                  <Flags>{subitem_flags}\n                  </Flags>\n                </SubItem>",
                    xml_escape(&subitem.name), esi.iec_name, esi.bitsize
                ));
                bits_offset += esi.bitsize;
            }
            result.push_str("\n              </DataType>");
            Ok(result)
        }
    }
}

/// `getSubitemFlags`, `esi_xml.js:111-121`.
fn subitem_flags(access: Option<Access>, pdo_flags: &str) -> String {
    match access {
        Some(a) => {
            let two = &a.macro_suffix()[..2];
            format!(
                "\n                    <Access WriteRestrictions=\"PreOP\">{}</Access>{pdo_flags}",
                two.to_lowercase()
            )
        }
        None => format!("\n                    <Access>ro</Access>{pdo_flags}"),
    }
}

/// `addVariableType`, `esi_xml.js:36-48`. `variable_types` is an
/// insertion-ordered dedup set (mirrors the JS plain object's insertion
/// order, which `Object.entries` later walks) keyed by the ESI type name
/// (e.g. `"UDINT"`, `"STRING(47)"`).
fn register_variable_type(dtype: Dtype, size: Option<u16>, variable_types: &mut IndexMap<String, u16>) {
    let name = esi_variable_type_name(dtype, size);
    variable_types.entry(name).or_insert_with(|| var_bitsize(dtype, size));
}

/// `esiVariableTypeName`, `esi_xml.js:356-362`.
fn esi_variable_type_name(dtype: Dtype, size: Option<u16>) -> String {
    let name = dtype.esi().iec_name;
    if dtype == Dtype::VisibleString {
        format!("{name}({})", size.unwrap_or(0))
    } else {
        name.to_string()
    }
}

/// `esiBitsize`, `esi_xml.js:377-404`.
fn esi_bitsize(objd: &Objd) -> u16 {
    match objd {
        Objd::Var { dtype, size, .. } => var_bitsize(dtype.expect("VAR objd without dtype"), *size),
        Objd::Array { dtype, items, .. } => {
            let maxsubindex_bitsize = Dtype::Unsigned8.esi().bitsize;
            let bitsize = dtype.expect("ARRAY objd without dtype").esi().bitsize;
            let elements = items.len() as u16 - 1;
            maxsubindex_bitsize * 2 + elements * bitsize
        }
        Objd::Record { items, .. } => {
            let maxsubindex_bitsize = Dtype::Unsigned8.esi().bitsize;
            let mut bitsize = maxsubindex_bitsize * 2;
            for subitem in items.iter().skip(1) {
                let d = subitem.dtype.expect("RECORD subitem without dtype");
                bitsize += d.esi().bitsize;
                if d == Dtype::Boolean {
                    bitsize += 7; // booleanPaddingBitsize, constants.js:61
                }
            }
            bitsize
        }
    }
}

/// `esiDtName`, `esi_xml.js:364-375`.
fn esi_dt_name(idx: u16, objd: &Objd) -> String {
    match objd {
        Objd::Var { dtype, size, .. } => esi_variable_type_name(dtype.expect("VAR objd without dtype"), *size),
        Objd::Array { .. } | Objd::Record { .. } => format!("DT{idx:X}"),
    }
}

// ####################### Objects section ####################### //

/// `addDictionaryObject`, `esi_xml.js:133-171`.
fn add_dictionary_object(idx: u16, objd: &Objd) -> Result<String, GenError> {
    let el_dtype = esi_dt_name(idx, objd);
    let bitsize = esi_bitsize(objd);
    let mut result = format!(
        "\n              <Object>\n                <Index>#x{idx:X}</Index>\n                <Name>{}</Name>\n                <Type>{el_dtype}</Type>\n                <BitSize>{bitsize}</BitSize>\n                <Info>",
        xml_escape(objd.name())
    );

    if let Some(v) = truthy_value(objd.value()) {
        if objd.dtype() == Some(Dtype::VisibleString) {
            result.push_str(&format!(
                "\n                  <DefaultString>{}</DefaultString>",
                xml_escape(&value_to_string(v))
            ));
        } else {
            result.push_str(&format!(
                "\n                  <DefaultValue>{}</DefaultValue>",
                xml_escape(&to_esi_hex_value(Some(v)))
            ));
        }
    }

    if !objd.items().is_empty() {
        result.push_str(&add_dictionary_object_subitems(objd.items()));
    }

    let mut flags = String::from("\n                  <Access>ro</Access>");
    if matches!(objd, Objd::Var { .. }) {
        flags.push_str(&pdo_mapping_flags(objd.pdo_mappings(), idx, objd.name())?);
    }
    if let Some(cat) = sdo_category(idx) {
        flags.push_str(&format!("\n                  <Category>{cat}</Category>"));
    }

    result.push_str(&format!(
        "\n                </Info>\n                <Flags>{flags}\n                </Flags>\n              </Object>"
    ));
    Ok(result)
}

/// `addDictionaryObjectSubitems`, `esi_xml.js:160-170`.
fn add_dictionary_object_subitems(items: &[SubItem]) -> String {
    let max_subindex_value = items.len() as u64 - 1;
    let mut result = String::new();
    for (subindex, subitem) in items.iter().enumerate() {
        let default_value = if subindex > 0 {
            to_esi_hex_value(subitem.value.as_ref())
        } else {
            to_esi_hex_value(Some(&Value::from(max_subindex_value)))
        };
        result.push_str(&format!(
            "\n                  <SubItem>\n                    <Name>{}</Name>\n                    <Info>\n                      <DefaultValue>{}</DefaultValue>\n                    </Info>\n                  </SubItem>",
            xml_escape(&subitem.name),
            xml_escape(&default_value)
        ));
    }
    result
}

/// `SDO_category`, `constants.js:119-122`: `{'1000': 'm', '1009': 'o'}`.
fn sdo_category(idx: u16) -> Option<&'static str> {
    match idx {
        0x1000 => Some("m"),
        0x1009 => Some("o"),
        _ => None,
    }
}

// ####################### PDOs ####################### //

/// `addEsiDevicePDO`, `esi_xml.js:223-274`.
fn add_esi_device_pdo(objd: &Objd, idx: u16, pdo_name: &str, mem_offset: u32) -> Result<String, GenError> {
    let pdo_letter = pdo_name.chars().next().expect("non-empty pdo name").to_ascii_uppercase();
    let sm_no = if pdo_name == "txpdo" { 3 } else { 2 };
    let mut esi = format!(
        "        <{pdo_letter}xPdo Fixed=\"true\" Mandatory=\"true\" Sm=\"{sm_no}\">\n          <Index>#x{mem_offset:X}</Index>\n          <Name>{}</Name>",
        xml_escape(objd.name())
    );

    match objd {
        Objd::Var { dtype, size, .. } => {
            let dtype = dtype.expect("VAR objd without dtype");
            let esi_type = esi_variable_type_name(dtype, *size);
            let bitsize = var_bitsize(dtype, *size);
            esi.push_str(&format!(
                "\n          <Entry>\n            <Index>#x{idx:X}</Index>\n            <SubIndex>#x0</SubIndex>\n            <BitLen>{bitsize}</BitLen>\n            <Name>{}</Name>\n            <DataType>{esi_type}</DataType>\n          </Entry>",
                xml_escape(objd.name())
            ));
            if dtype == Dtype::Boolean {
                esi.push_str(&pdo_boolean_padding());
            }
        }
        Objd::Array { dtype, items, size, .. } => {
            let dtype = dtype.expect("ARRAY objd without dtype");
            let esi_type = esi_variable_type_name(dtype, *size);
            let bitsize = var_bitsize(dtype, *size);
            for (subindex, subitem) in items.iter().enumerate().skip(1) {
                esi.push_str(&format!(
                    "\n          <Entry>\n            <Index>#x{idx:X}</Index>\n            <SubIndex>#x{subindex:x}</SubIndex>\n            <BitLen>{bitsize}</BitLen>\n            <Name>{}</Name>\n            <DataType>{esi_type}</DataType>\n          </Entry>",
                    xml_escape(&subitem.name)
                ));
                // TODO (reference too): array-of-booleans padding unhandled.
            }
        }
        Objd::Record { items, .. } => {
            for (subindex, subitem) in items.iter().enumerate().skip(1) {
                let sd = subitem.dtype.expect("RECORD subitem without dtype");
                let esi_type = esi_variable_type_name(sd, None);
                let bitsize = var_bitsize(sd, None);
                esi.push_str(&format!(
                    "\n          <Entry>\n            <Index>#x{idx:X}</Index>\n            <SubIndex>#x{subindex:x}</SubIndex>\n            <BitLen>{bitsize}</BitLen>\n            <Name>{}</Name>\n            <DataType>{esi_type}</DataType>\n          </Entry>",
                    xml_escape(&subitem.name)
                ));
                if sd == Dtype::Boolean {
                    esi.push_str(&pdo_boolean_padding());
                }
            }
        }
    }

    esi.push_str(&format!("\n        </{pdo_letter}xPdo>\n"));
    Ok(esi)
}

/// `pdoBooleanPadding`, `esi_xml.js:268-273`.
fn pdo_boolean_padding() -> String {
    "\n          <Entry>\n            <Index>0</Index>\n            <SubIndex>0</SubIndex>\n            <BitLen>7</BitLen>\n          </Entry>".to_string()
}

/// `getPdoMappingFlags`, `esi_xml.js:286-297`. **Divergence:** the JS
/// reference `alert()`s on `pdo_mappings.length > 1` and silently proceeds
/// using only the first mapping; this port errors instead.
fn pdo_mapping_flags(mappings: &[String], idx: u16, name: &str) -> Result<String, GenError> {
    if mappings.is_empty() {
        return Ok(String::new());
    }
    if mappings.len() > 1 {
        return Err(GenError::Od(format!(
            "object {name:?} (0x{idx:X}) has multiple PDO mappings, that is not supported"
        )));
    }
    let flag = mappings[0].chars().next().expect("non-empty pdo mapping name").to_ascii_uppercase();
    Ok(format!("\n                  <PdoMapping>{flag}</PdoMapping>"))
}

// ####################### Mailbox / DC ####################### //

/// `getCoEMailboxSection`, `esi_xml.js:326-354`.
fn coe_mailbox_attrs(config: &Config) -> String {
    format!(
        "SdoInfo=\"{}\" PdoAssign=\"{}\" PdoConfig=\"{}\" PdoUpload=\"{}\" CompleteAccess=\"{}\" ",
        config.coe_details_enable_sdo_info,
        config.coe_details_enable_pdo_assign,
        config.coe_details_enable_pdo_configuration,
        config.coe_details_enable_upload_at_startup,
        config.coe_details_enable_sdo_complete_access,
    )
}

/// `getEsiDCsection`, `esi_xml.js:299-323`.
fn dc_section(dc: &[SyncMode]) -> String {
    if dc.is_empty() {
        return String::new();
    }
    let mut s = String::from("        <Dc>");
    for op in dc {
        s.push_str(&format!(
            "\n          <OpMode>\n            <Name>{}</Name>\n            <Desc>{}</Desc>\n            <AssignActivate>{}</AssignActivate>",
            xml_escape(&op.name),
            xml_escape(&op.description),
            xml_escape(&op.assign_activate)
        ));
        if dc_field_active(&op.sync0_cycle_time) {
            s.push_str(&format!(
                "\n            <CycleTimeSync0>{}</CycleTimeSync0>",
                xml_escape(&op.sync0_cycle_time)
            ));
        }
        if dc_field_active(&op.sync0_shift_time) {
            s.push_str(&format!(
                "\n            <ShiftTimeSync0>{}</ShiftTimeSync0>",
                xml_escape(&op.sync0_shift_time)
            ));
        }
        if dc_field_active(&op.sync1_cycle_time) {
            s.push_str(&format!(
                "\n            <CycleTimeSync1>{}</CycleTimeSync1>",
                xml_escape(&op.sync1_cycle_time)
            ));
        }
        if dc_field_active(&op.sync1_shift_time) {
            s.push_str(&format!(
                "\n            <ShiftTimeSync1>{}</ShiftTimeSync1>",
                xml_escape(&op.sync1_shift_time)
            ));
        }
        s.push_str("\n          </OpMode>");
    }
    s.push_str("\n        </Dc>\n");
    s
}

/// `opMode.SyncNcycleTime && opMode.SyncNcycleTime != 0`: truthy (non-empty
/// string) AND numerically non-zero after loose JS coercion. A non-numeric
/// string is `NaN != 0` (`true`) under that coercion, so a parse failure
/// counts as active too.
fn dc_field_active(s: &str) -> bool {
    !s.is_empty() && s.trim().parse::<f64>().map(|n| n != 0.0).unwrap_or(true)
}

// ####################### value formatting ####################### //

/// JS truthiness for the `Value` shapes that ever appear in `objd.value`/
/// `subitem.value` (string or number): `0`, `""`, `null` are falsy;
/// everything else (including the string `"0"`) is truthy.
fn value_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn truthy_value(v: Option<&Value>) -> Option<&Value> {
    v.filter(|v| value_truthy(v))
}

/// `${value}` JS template-literal stringification.
fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// `toEsiHexValue`, `esi_xml.js:276-284`.
fn to_esi_hex_value(v: Option<&Value>) -> String {
    match truthy_value(v) {
        None => "0".to_string(),
        Some(Value::String(s)) => match s.strip_prefix("0x") {
            Some(rest) => format!("#x{rest}"),
            None => s.clone(),
        },
        Some(other) => value_to_string(other),
    }
}
