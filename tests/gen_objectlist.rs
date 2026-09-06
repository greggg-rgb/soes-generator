mod common;
use soes_generator::{generators::objectlist, model::{Objd, Project, SubItem}, od_build::build_object_dictionary, types::Dtype};

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

fn wide_items(prefix: &str) -> Vec<SubItem> {
    (0..=16)
        .map(|i| SubItem {
            name: format!("{prefix} {i}"),
            dtype: Some(Dtype::Unsigned8),
            value: None,
            access: None,
            data: None,
        })
        .collect()
}

fn assert_numeric_subindices(output: &str, index: u16) {
    for i in 1..=16 {
        let expected = format!("0x{i:02X}, DTYPE_UNSIGNED8, 8, ATYPE_RO, acName{index:X}_{i:02},");
        assert!(output.contains(&expected), "missing {expected} in:\n{output}");
    }
}

#[test]
fn objectlist_array_uses_hex_numeric_subindices() {
    let p = Project::builder()
        .add_sdo(Objd::array(0x2000, Dtype::Unsigned8, "Wide array", wide_items("array")))
        .build();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    assert_numeric_subindices(&objectlist::generate(&p.config, &od), 0x2000);
}

#[test]
fn objectlist_record_uses_hex_numeric_subindices() {
    let p = Project::builder()
        .add_sdo(Objd::record(0x2001, "Wide record", wide_items("record")))
        .build();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    assert_numeric_subindices(&objectlist::generate(&p.config, &od), 0x2001);
}

#[test]
fn real64_default_value_passes_through() {
    // spec Testing §2 (REAL64 is not a bug — default branch, objectlist.js:192)
    let p = Project::builder().add_txpdo(Objd::var_with_value(0x6000, Dtype::Real64, "d", "2.5")).build();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    assert!(objectlist::generate(&p.config, &od).contains("2.5"));
}
