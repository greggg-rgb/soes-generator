mod common;
use soes_generator::{generate, model::Project};

#[test]
fn generate_default_fills_all_bundle_fields_from_goldens() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap())
        .unwrap();
    let b = generate(&p).unwrap();
    common::assert_eq_lines(&b.objectlist_c, &common::load_golden("default", "objectlist.c"));
    common::assert_eq_lines(&b.utypes_h, &common::load_golden("default", "utypes.h"));
    common::assert_eq_lines(&b.ecat_options_h, &common::load_golden("default", "ecat_options.h"));
    common::assert_eq_lines(&b.esi_xml, &common::load_golden("default", "device.xml"));
    assert_eq!(b.eeprom_bin, std::fs::read("tests/golden/default/eeprom.bin").unwrap());
    common::assert_eq_lines(&b.eeprom_hex, &common::load_golden("default", "eeprom.hex"));
    common::assert_eq_lines(&b.eeprom_h, &common::load_golden("default", "eeprom.h"));
}
