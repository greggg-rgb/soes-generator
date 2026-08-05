//! Golden-vector test helpers.
//!
//! `tests/golden/**` is dumped verbatim from the JS reference generator
//! (see `scripts/dump_golden.js`) via `.cache/EEPROM_generator`, commit
//! 6f26c1b. `objectlist.c`/`utypes.h`/`ecat_options.h`/`eeprom.*`/
//! `configdata.txt` are byte-identical to what the JS reference produces.
//!
//! The ONLY intentional divergence is in every `tests/golden/*/device.xml`:
//! `Physics="YY "` (JS, 1 trailing space) was hand-patched to
//! `Physics="YY  "` (2 trailing spaces) for the default-port fixtures
//! (default/foe/cia402). This reflects bug 4 being fixed in the Rust port:
//! the JS ESI generator only emits the first 2 configured ports, dropping
//! trailing unconfigured ports from the `Physics` attribute; the Rust port
//! emits all 4 ports as specified by ETG.2000, so ports 2-3 (both " " in
//! these fixtures) contribute their own characters instead of being
//! truncated away. No fixture contains `& < >`, so XML escaping does not
//! otherwise alter any golden.
use std::path::Path;

pub fn load_golden(fixture: &str, file: &str) -> String {
    std::fs::read_to_string(Path::new("tests/golden").join(fixture).join(file))
        .unwrap_or_else(|e| panic!("missing golden {fixture}/{file}: {e}"))
}

pub fn assert_eq_lines(actual: &str, expected: &str) {
    for (i, (a, e)) in actual.lines().zip(expected.lines()).enumerate() {
        assert_eq!(a, e, "first diff at line {}", i + 1);
    }
    assert_eq!(
        actual.lines().count(),
        expected.lines().count(),
        "line count differs"
    );
}
