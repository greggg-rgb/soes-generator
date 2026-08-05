use soes_generator::{model::Project, od_build::build_object_dictionary};

#[test]
fn empty_project_has_mandatory_objects() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    for idx in [0x1000, 0x1008, 0x1009, 0x100A, 0x1018, 0x1C00] {
        assert!(od.contains_key(&idx), "missing {idx:#X}");
    }
    // numeric ordering (BTreeMap): keys ascend
    let keys: Vec<u16> = od.keys().copied().collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]));

    // structural values (mirror odSpecs.js getExpectedEmptyOd, odSpecs.js:7-28):
    // 1000 stays constant 0x1389 (5001 decimal)
    let device_type = &od[&0x1000];
    assert_eq!(device_type.value().and_then(|v| v.as_u64()), Some(0x1389));

    // 1018 is a RECORD with a Max SubIndex placeholder at items[0], and 4
    // more sub-items populated from the (default) config identity fields.
    let identity = &od[&0x1018];
    assert_eq!(identity.items().len(), 5);
    assert_eq!(identity.items()[0].name, "Max SubIndex");
    assert_eq!(identity.items()[1].value.as_ref().unwrap().as_u64(), Some(0x000)); // VendorID "0x000"

    // 1C00 array items are 1,2,3,4 (Max SubIndex + 4 SM comm types)
    let sm_comm = &od[&0x1C00];
    assert_eq!(sm_comm.items().len(), 5);
    let vals: Vec<u64> = sm_comm.items()[1..]
        .iter()
        .map(|i| i.value.as_ref().unwrap().as_u64().unwrap())
        .collect();
    assert_eq!(vals, vec![1, 2, 3, 4]);

    // 1008/1009/100A populated from device/HW/SW version strings
    let dev_name = &od[&0x1008];
    assert_eq!(dev_name.value().and_then(|v| v.as_str()), Some("2-channel Hypergalactic input superimpermanator"));
    assert_eq!(dev_name.size(), Some("2-channel Hypergalactic input superimpermanator".len() as u16));
}

#[test]
fn cia402_synthesizes_sm_assignment_and_mappings() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/cia402.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    assert!(od.contains_key(&0x1C12) || od.contains_key(&0x1C13)); // SM assignment synthesized
    // both TX and RX PDOs exist in the fixture, so both assignment arrays must exist
    assert!(od.contains_key(&0x1C12));
    assert!(od.contains_key(&0x1C13));

    // TX PDO objects live at SM3Offset (0x1A00) onward; the mapping record's
    // name matches the mapped object's name.
    let tx_mapping = &od[&0x1A00];
    assert_eq!(tx_mapping.name(), "Status Word");
    assert_eq!(tx_mapping.items().len(), 2); // Max SubIndex + 1 mapping entry (VAR)

    // SM assignment array carries one "PDO Mapping" sub-item per mapped object
    let tx_assign = &od[&0x1C13];
    assert_eq!(tx_assign.items().len(), 3); // Max SubIndex + 2 TXPDO objects

    let rx_assign = &od[&0x1C12];
    assert_eq!(rx_assign.items().len(), 3); // Max SubIndex + 2 RXPDO objects

    // the "A" stray key (duplicate name "Error Settings" with 10F1) must be
    // silently excluded by getUsedIndexes-equivalent range filtering, or the
    // duplicate-name validation would incorrectly reject this fixture.
    assert!(!od.contains_key(&0x000A));
}

#[test]
fn validation_rejects_duplicate_names() {
    // spec §Model validation set
    use soes_generator::{
        model::{Objd, Project},
        types::Dtype,
        GenError,
    };
    let p = Project::builder() // builder seeds a VALID default Config (Task 5), so this reaches the dup check
        .add_sdo(Objd::var(0x2000, Dtype::Unsigned32, "dup"))
        .add_txpdo(Objd::var(0x6000, Dtype::Unsigned32, "dup"))
        .build();
    let err = build_object_dictionary(&p.config, &p.od).unwrap_err();
    assert!(matches!(err, GenError::Od(ref m) if m.contains("dup")), "expected dup-name Od error, got {err:?}");
}

#[test]
fn validation_rejects_pdo_mapped_disallowed_dtype() {
    use soes_generator::{
        model::{Objd, Project},
        types::Dtype,
    };
    // VISIBLE_STRING is NOT in dtypes_PDO_allowed (constants.js:93) -> PDO-mapping it must error
    let p = Project::builder().add_txpdo(Objd::var(0x6000, Dtype::VisibleString, "s")).build();
    assert!(build_object_dictionary(&p.config, &p.od).is_err());
}

#[test]
fn validation_rejects_short_visible_string_size() {
    use soes_generator::model::{Objd, Project};
    // size 2 < value length 4 -> error (VISIBLE_STRING size >= value.len())
    let p = Project::builder().add_sdo(Objd::var_string(0x2000, "s", "abcd", 2)).build();
    assert!(build_object_dictionary(&p.config, &p.od).is_err());
}
