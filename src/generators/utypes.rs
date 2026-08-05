//! `utypes.h` generator — port of `.cache/EEPROM_generator/src/generators/utypes.js`.
//!
//! Emits the `_Objects` struct holding the slave's backing storage: an
//! Identity `serial` field, then Inputs (TxPDO-mapped objects), Outputs
//! (RxPDO-mapped objects), and Parameters (SDO items — `is_sdo_item`, not
//! "unmapped", since mandatory VARs are also unmapped).

use crate::model::{Config, Objd};
use crate::names::variable_name;
use crate::od_build::Od;
use crate::types::{Dtype, PdoDir};

pub fn generate(_config: &Config, od: &Od) -> String {
    let mut out = String::from(
        "#ifndef __UTYPES_H__\n#define __UTYPES_H__\n\n#include \"cc.h\"\n\n\
         /* Object dictionary storage */\n\ntypedef struct\n{\n   /* Identity */\n",
    );
    out.push_str("\n   uint32_t serial;\n");

    let mut inputs = String::from("\n   /* Inputs */\n");
    let mut outputs = String::from("\n   /* Outputs */\n");
    let mut has_inputs = false;
    let mut has_outputs = false;

    for objd in od.values() {
        match objd.pdo_dir() {
            PdoDir::Tx => {
                inputs.push_str(&declaration(objd));
                has_inputs = true;
            }
            PdoDir::Rx => {
                outputs.push_str(&declaration(objd));
                has_outputs = true;
            }
            PdoDir::None => {}
        }
    }

    if has_inputs {
        out.push_str(&inputs);
        out.push('\n');
    }
    if has_outputs {
        out.push_str(&outputs);
        out.push('\n');
    }

    let mut parameters = String::from("\n   /* Parameters */\n");
    let mut any_parameters = false;
    for objd in od.values() {
        if objd.is_sdo_item() {
            parameters.push_str(&declaration(objd));
            any_parameters = true;
        }
    }
    if any_parameters {
        out.push_str(&parameters);
    }

    out.push_str("\n} _Objects;\n\nextern _Objects Obj;\n\n#endif /* __UTYPES_H__ */\n");
    out
}

fn declaration(objd: &Objd) -> String {
    let var_name = variable_name(objd.name());
    match objd {
        Objd::Var { dtype, size, .. } => {
            let dtype = dtype.expect("VAR objd without dtype");
            let ctype = dtype.esi().ctype;
            let suffix = if dtype == Dtype::VisibleString {
                format!("[{}]", size.unwrap_or(0))
            } else {
                String::new()
            };
            format!("\n   {ctype} {var_name}{suffix};")
        }
        Objd::Array { dtype, items, .. } => {
            let ctype = dtype.expect("ARRAY objd without dtype").esi().ctype;
            format!("\n   {ctype} {var_name}[{}];", items.len() - 1)
        }
        Objd::Record { items, .. } => {
            let mut section = String::from("\n   struct\n   {");
            for subitem in items.iter().skip(1) {
                let sub_ctype = subitem.dtype.expect("RECORD subitem without dtype").esi().ctype;
                let sub_name = variable_name(&subitem.name);
                section.push_str(&format!("\n      {sub_ctype} {sub_name};"));
            }
            section.push_str(&format!("\n   }} {var_name};"));
            section
        }
    }
}
