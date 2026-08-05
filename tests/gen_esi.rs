mod common;
use soes_generator::{
    generators::{eeprom, esi},
    model::Project,
    od_build::build_object_dictionary,
};

fn load_fixture(name: &str) -> Project {
    Project::from_json(&std::fs::read_to_string(format!("tests/fixtures/{name}.json")).unwrap()).unwrap()
}

#[test]
fn esi_default_matches_patched_golden() {
    let p = load_fixture("default");
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    let out = esi::generate(&p.config, &od, &p.dc).unwrap();
    common::assert_eq_lines(&out, &common::load_golden("default", "device.xml"));
}

#[test]
fn esi_cia402_matches_patched_golden() {
    let p = load_fixture("cia402");
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    let out = esi::generate(&p.config, &od, &p.dc).unwrap();
    common::assert_eq_lines(&out, &common::load_golden("cia402", "device.xml"));
}

#[test]
fn esi_foe_matches_patched_golden() {
    let p = load_fixture("foe");
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    let out = esi::generate(&p.config, &od, &p.dc).unwrap();
    common::assert_eq_lines(&out, &common::load_golden("foe", "device.xml"));
}

/// BUG-4 regression: the JS reference (`esi_xml.js:26`) drops Port3Physical
/// from the `Physics` attribute due to an operator-precedence bug
/// (`(P0+P1+P2) || (+P3)`). This port must emit all four ports.
#[test]
fn physics_includes_all_four_ports() {
    let mut p = load_fixture("default");
    p.config.port0_physical = "Y".into();
    p.config.port1_physical = "Y".into();
    p.config.port2_physical = "Y".into();
    p.config.port3_physical = "H".into();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    let out = esi::generate(&p.config, &od, &p.dc).unwrap();
    assert!(out.contains("Physics=\"YYYH\""), "got:\n{out}");
}

/// A device name containing `&` must come out as `&amp;`, never as a raw
/// `&` (which would produce invalid XML).
#[test]
fn xml_text_is_escaped() {
    let mut p = load_fixture("default");
    p.config.text_device_name = "Foo & Bar".into();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    let out = esi::generate(&p.config, &od, &p.dc).unwrap();
    assert!(out.contains("Foo &amp; Bar"), "got:\n{out}");
    assert!(!out.contains("Foo & Bar"), "raw & leaked into XML:\n{out}");
}

#[test]
fn config_data_string_matches_esc_golden() {
    let p = load_fixture("default"); // ET1100
    let out = eeprom::config_data_string(&p.config).unwrap();
    assert_eq!(out, common::load_golden("esc", "et1100.txt").trim());
}

/// Multiple PDO mappings on one object must be a hard error, not a silent
/// "use the first one" (the JS reference's behavior via `alert()`).
#[test]
fn multiple_pdo_mappings_is_an_error() {
    use soes_generator::model::Objd;
    use soes_generator::types::Dtype;

    let mut p = Project::builder()
        .add_rxpdo(Objd::var(0x6000, Dtype::Unsigned16, "flag"))
        .build();
    p.od.rxpdo.get_mut("6000").unwrap().add_pdo_mapping("txpdo");
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    let err = esi::generate(&p.config, &od, &p.dc).unwrap_err();
    assert!(err.to_string().contains("multiple PDO mappings"), "got: {err}");
}
