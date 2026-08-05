mod common;
use soes_generator::{generators::ecat_options, model::{Objd, Project}, od_build::build_object_dictionary, types::Dtype};

#[test]
fn ecat_options_default_matches_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    common::assert_eq_lines(&ecat_options::generate(&p.config, &od), &common::load_golden("default", "ecat_options.h"));
}

#[test]
fn ecat_options_cia402_matches_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/cia402.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    common::assert_eq_lines(&ecat_options::generate(&p.config, &od), &common::load_golden("cia402", "ecat_options.h"));
}

#[test]
fn ecat_options_foe_matches_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/foe.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    common::assert_eq_lines(&ecat_options::generate(&p.config, &od), &common::load_golden("foe", "ecat_options.h"));
}

#[test]
fn boolean_pdo_counts_as_two_mappings() {
    // ecat_options.js:76 (+1 padding); od.js booleanPaddingBitsize=7
    let p = Project::builder().add_txpdo(Objd::var(0x6000, Dtype::Boolean, "flag")).build();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    // one BOOLEAN VAR -> data entry + 7-bit padding entry -> MAX_MAPPINGS_SM3 == 2
    let out = ecat_options::generate(&p.config, &od);
    assert!(out.contains("#define MAX_MAPPINGS_SM3 2"), "got:\n{out}");
}
