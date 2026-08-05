mod common;
use soes_generator::{generators::utypes, model::Project, od_build::build_object_dictionary};

#[test]
fn utypes_default_matches_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    common::assert_eq_lines(&utypes::generate(&p.config, &od), &common::load_golden("default", "utypes.h"));
}

#[test]
fn utypes_cia402_matches_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/cia402.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    common::assert_eq_lines(&utypes::generate(&p.config, &od), &common::load_golden("cia402", "utypes.h"));
}

#[test]
fn integer64_uses_numeric_bitsize_and_int64_ctype() {
    // guards the JS string-'64' footgun
    use soes_generator::{model::Objd, types::Dtype};
    let p = Project::builder().add_txpdo(Objd::var(0x6000, Dtype::Integer64, "big")).build();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    assert!(utypes::generate(&p.config, &od).contains("int64_t big;"));
}
