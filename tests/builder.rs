use soes_generator::model::{Objd, Project, SubItem};
use soes_generator::types::{Dtype, PdoDir};

#[test]
fn builder_places_objects_by_section() {
    let p = Project::builder()
        .add_txpdo(Objd::var(0x6000, Dtype::Unsigned32, "input"))
        .build();
    assert!(p.od.txpdo.contains_key("6000"));
    assert_eq!(p.od.txpdo["6000"].pdo_dir(), PdoDir::Tx);
}

#[test]
fn add_rxpdo_and_add_sdo_place_and_tag_correctly() {
    let p = Project::builder()
        .add_rxpdo(Objd::var(0x7000, Dtype::Unsigned16, "output"))
        .add_sdo(Objd::var_with_value(0x2000, Dtype::Real32, "gain", "1.5"))
        .build();

    assert!(p.od.rxpdo.contains_key("7000"));
    assert_eq!(p.od.rxpdo["7000"].pdo_dir(), PdoDir::Rx);

    assert!(p.od.sdo.contains_key("2000"));
    assert_eq!(p.od.sdo["2000"].pdo_dir(), PdoDir::None);
}

#[test]
fn var_string_sets_visible_string_dtype_and_size() {
    let p = Project::builder()
        .add_sdo(Objd::var_string(0x1008, "Device Name", "MyDevice", 32))
        .build();
    let Objd::Var { dtype, size, value, .. } = &p.od.sdo["1008"] else {
        panic!("expected Var")
    };
    assert_eq!(*dtype, Some(Dtype::VisibleString));
    assert_eq!(*size, Some(32));
    assert_eq!(value.as_ref().unwrap().as_str(), Some("MyDevice"));
}

/// ARRAY/RECORD are first-class od entries (od_build/utypes/objectlist all
/// handle them, and the cia402 fixture contains them) — smoke-test that the
/// array/record constructors land in the right section, so they aren't
/// silently untested.
#[test]
fn array_and_record_land_in_sdo_section() {
    let items = vec![SubItem {
        name: "Max SubIndex".into(),
        dtype: None,
        value: None,
        access: None,
        data: None,
    }];
    let p = Project::builder()
        .add_sdo(Objd::array(
            0x1C00,
            Dtype::Unsigned8,
            "Sync Manager Communication Type",
            items.clone(),
        ))
        .add_sdo(Objd::record(0x1018, "Identity Object", items))
        .build();

    assert!(matches!(p.od.sdo["1C00"], Objd::Array { .. }));
    assert!(matches!(p.od.sdo["1018"], Objd::Record { .. }));
}

/// Load-bearing: the builder MUST seed `config` with the JS form defaults
/// (`getFormDefaultValues().form`, constants.js:183-218), not empty-string
/// `Config::default()` — downstream builder tests call
/// `build_object_dictionary` which parses identity fields via `parse_u32`
/// and panics on empty strings. This asserts the seed matches
/// `tests/fixtures/default.json` exactly (that fixture IS
/// `getFormDefaultValues().form`, captured by Task 4), so the hardcoded
/// seed can't silently drift from the JS reference.
#[test]
fn builder_seeds_config_with_form_defaults_matching_fixture() {
    let built = Project::builder().build();
    let fixture = Project::from_json(
        &std::fs::read_to_string("tests/fixtures/default.json").expect("read fixture"),
    )
    .expect("parse fixture");
    assert_eq!(built.config, fixture.config);
}
