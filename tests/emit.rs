mod common;
use soes_generator::{emit, model::Project, names};

#[test]
fn emit_writes_all_eight_files() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap())
        .unwrap();

    let out_dir = std::env::temp_dir().join(format!("soes_emit_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir); // best-effort pre-clean

    let result = emit(&p, &out_dir);
    let emitted = match result {
        Ok(e) => e,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&out_dir);
            panic!("emit failed: {e}");
        }
    };

    let expected_xml_name = format!("{}.xml", names::variable_name(&p.config.text_device_name));
    let expected_files = [
        "objectlist.c",
        "utypes.h",
        "ecat_options.h",
        "eeprom.bin",
        "eeprom.hex",
        "eeprom.h",
        "esi.json",
        expected_xml_name.as_str(),
    ];

    for name in expected_files {
        let path = out_dir.join(name);
        assert!(path.exists(), "expected file {} to exist", path.display());
    }

    assert_eq!(emitted.paths.len(), 8);
    let xml_path = emitted
        .paths
        .iter()
        .find(|p| p.file_name().unwrap() == expected_xml_name.as_str());
    assert!(xml_path.is_some(), "emitted paths should include the device xml");

    let _ = std::fs::remove_dir_all(&out_dir); // best-effort cleanup
}
