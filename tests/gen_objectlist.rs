mod common;
use soes_generator::{generators::objectlist, model::{Objd, Project}, od_build::build_object_dictionary, types::Dtype};

#[test]
fn objectlist_default_matches_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    common::assert_eq_lines(&objectlist::generate(&p.config, &od), &common::load_golden("default", "objectlist.c"));
}

#[test]
fn objectlist_cia402_matches_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/cia402.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    common::assert_eq_lines(&objectlist::generate(&p.config, &od), &common::load_golden("cia402", "objectlist.c"));
}

#[test]
fn real32_default_value_is_encoded_not_zeroed() {
    // bug-1 regression (reference emits 0x00000000 for every REAL32 default)
    let p = Project::builder().add_txpdo(Objd::var_with_value(0x6000, Dtype::Real32, "f", "1.5")).build();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    let out = objectlist::generate(&p.config, &od);
    assert!(out.contains("0x3FC00000"), "REAL32 1.5 must encode IEEE-754, got:\n{out}");
}

#[test]
fn real64_default_value_passes_through() {
    // spec Testing §2 (REAL64 is not a bug — default branch, objectlist.js:192)
    let p = Project::builder().add_txpdo(Objd::var_with_value(0x6000, Dtype::Real64, "d", "2.5")).build();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    assert!(objectlist::generate(&p.config, &od).contains("2.5"));
}
