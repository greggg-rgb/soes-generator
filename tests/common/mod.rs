//! Golden-vector test helpers.
//!
//! `tests/golden/**` is dumped from the JS reference generator (see
//! `scripts/dump_golden.js`) via `.cache/EEPROM_generator`, commit 6f26c1b.
//! `objectlist.c`/`utypes.h`/`ecat_options.h`/`eeprom.bin`/`eeprom.hex`/
//! `configdata.txt` are byte-identical to what the JS reference produces.
//!
//! Two files carry an intentional divergence, applied by `dump_golden.js`'s
//! `patchDivergences()` so the dump reproduces these goldens rather than
//! reverting the port's fixes:
//!
//! 1. every `tests/golden/*/device.xml`: `Physics="YY "` (JS, 1 trailing
//!    space) → `Physics="YY  "` (2 spaces) for the default-port fixtures
//!    (default/foe/cia402). This is bug 4 fixed: the JS ESI generator only
//!    emits the first 2 configured ports, dropping trailing ports from
//!    `Physics`; the Rust port emits all 4 (ETG.2000), so ports 2-3 (both
//!    " " here) contribute their own characters instead of being truncated.
//! 2. every `tests/golden/*/eeprom.h`: the trailing `#endif __ESI_EEPROM_H__`
//!    (JS bare token, non-conformant under `-pedantic`) → the conformant
//!    `#endif /* __ESI_EEPROM_H__ */`. See docs/faithful-port-quirks.md #2.
//! 3. every `tests/golden/*/eeprom.hex`: the checksum byte is zero-padded to
//!    two hex digits. The JS reference's `.slice(-2)` left a single digit when
//!    the checksum was below 0x10 (one such record per fixture, at address
//!    0x0100), producing a malformed Intel-HEX line; the Rust port emits a
//!    conformant 2-digit checksum. See docs/faithful-port-quirks.md #1.
//!
//! No fixture contains `& < >`, so XML escaping does not otherwise alter any
//! golden.
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
