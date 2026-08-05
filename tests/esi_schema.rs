//! Validates the generated ESI XML against the official ETG.2000 schema
//! (`tests/schema/EtherCATInfo.xsd`) using `xmllint`.
//!
//! `xmllint` (from libxml2-utils) is the standard EtherCAT ESI validator. If
//! it isn't installed, these tests SKIP rather than fail, so `cargo test`
//! stays green on machines without it; CI installs it and enforces the check
//! (see `.github/workflows/ci.yml`). All three fixtures are verified to pass
//! real `xmllint --schema` — a schema regression in the generator fails here.
use soes_generator::{generators::esi, model::Project, od_build::build_object_dictionary};
use std::io::Write;
use std::process::{Command, Stdio};

const SCHEMA: &str = "tests/schema/EtherCATInfo.xsd";

/// Runs `xmllint --noout --schema <SCHEMA> -`, feeding `xml` on stdin.
/// Returns `None` if `xmllint` is not installed (test should skip);
/// otherwise `Some((ok, stderr))` where `ok` is schema-valid.
fn xmllint_validate(xml: &str) -> Option<(bool, String)> {
    let mut child = match Command::new("xmllint")
        .args(["--noout", "--schema", SCHEMA, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        // NotFound => xmllint absent; any other spawn error is a real problem.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => panic!("failed to spawn xmllint: {e}"),
    };
    child.stdin.take().unwrap().write_all(xml.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    Some((out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned()))
}

fn esi_for(fixture: &str) -> String {
    let p = Project::from_json(&std::fs::read_to_string(format!("tests/fixtures/{fixture}.json")).unwrap())
        .unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    esi::generate(&p.config, &od, &p.dc).unwrap()
}

fn assert_valid(fixture: &str) {
    let xml = esi_for(fixture);
    match xmllint_validate(&xml) {
        None => eprintln!("SKIP {fixture}: xmllint not installed (install libxml2-utils to run this check)"),
        Some((true, _)) => {}
        Some((false, stderr)) => panic!("ESI for '{fixture}' failed schema validation:\n{stderr}"),
    }
}

#[test]
fn esi_default_is_schema_valid() {
    assert_valid("default");
}

#[test]
fn esi_foe_is_schema_valid() {
    assert_valid("foe");
}

#[test]
fn esi_cia402_is_schema_valid() {
    assert_valid("cia402");
}

/// Sanity check that the validation actually rejects bad XML — otherwise a
/// broken `xmllint` invocation would let every test pass vacuously.
#[test]
fn schema_rejects_malformed_esi() {
    match xmllint_validate("<EtherCATInfo><NotAValidChild/></EtherCATInfo>") {
        None => eprintln!("SKIP: xmllint not installed"),
        Some((ok, _)) => assert!(!ok, "xmllint accepted structurally invalid ESI — check harness"),
    }
}
