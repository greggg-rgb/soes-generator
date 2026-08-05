use soes_generator::model::Project;

#[test]
fn cia402_roundtrips_semantically() {
    let src = std::fs::read_to_string("tests/fixtures/cia402.json").unwrap();
    let p = Project::from_json(&src).expect("deserialize");
    // spot-check load fidelity
    assert_eq!(p.config.vendor_id, "0x1337"); // cia402 fixture VendorID
    assert!(p.od.txpdo.len() + p.od.rxpdo.len() > 0);
    // semantic round-trip: reload our re-serialization, compare models
    let again = Project::from_json(&p.to_json()).unwrap();
    assert_eq!(p, again); // derive PartialEq on the model
}
