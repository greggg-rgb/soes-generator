pub mod types;
pub mod names;
pub mod model;
pub mod od_build;
pub mod generators;

use std::path::Path;
use model::Project;

#[derive(Debug, thiserror::Error)]
pub enum GenError {
    #[error("invalid config field {field}: {msg}")]
    Config { field: &'static str, msg: String },
    #[error("object dictionary error: {0}")]
    Od(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct Bundle {
    pub objectlist_c: String,
    pub utypes_h: String,
    pub ecat_options_h: String,
    pub esi_xml: String,
    pub eeprom_bin: Vec<u8>,
    pub eeprom_hex: String,
    pub eeprom_h: String,
    pub backup_json: String,
}

pub struct Emitted { pub paths: Vec<std::path::PathBuf> }

pub fn generate(p: &Project) -> Result<Bundle, GenError> {
    let od = od_build::build_object_dictionary(&p.config, &p.od)?;
    let bin = generators::eeprom::hex_generator(&p.config)?;
    let hex = generators::intel_hex::to_intel_hex(&bin)?;
    let eeprom_h = generators::intel_hex::to_esi_eeprom_h(&bin);
    Ok(Bundle {
        objectlist_c: generators::objectlist::generate(&p.config, &od),
        utypes_h: generators::utypes::generate(&p.config, &od),
        ecat_options_h: generators::ecat_options::generate(&p.config, &od),
        esi_xml: generators::esi::generate(&p.config, &od, &p.dc)?,
        eeprom_bin: bin,
        eeprom_hex: hex,
        eeprom_h,
        backup_json: p.to_json(),
    })
}
pub fn emit(_p: &Project, _out_dir: &Path) -> Result<Emitted, GenError> { todo!() }
