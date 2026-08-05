# Feasibility & Rust port plan

## Verdict

**Feasible and low-risk.** The reference is a self-contained, deterministic
`config → files` transform: ~3.1k LOC of pure JS, no async, no I/O in the core, no
external services. There is a passing 73-spec suite whose golden outputs become exact
regression vectors, and the JS reference can be run headlessly as a live oracle for any
new input. The only genuinely tricky work is byte-exact reproduction of the SII binary
and the shared C-ABI contract — both fully specified by existing golden tests.

## Why it ports cleanly

| Property | Consequence for the port |
|----------|--------------------------|
| Pure function, no runtime state | Rust core is just functions; no lifetimes/async complexity |
| Deterministic, golden-tested output | TDD against lifted `toEqualLines` strings; byte-for-byte diff |
| JS reference runs headless (73 specs pass) | Live oracle — generate the same input in JS and Rust, diff |
| Serialization = plain JSON (`backup.js`) | `serde` mirrors the `{form, od, dc}` shape directly |
| Small type surface | `ESI_DT` + a dozen constants port verbatim |

## Risks & mitigations

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| SII byte/CRC mismatch (silent — ESC just rejects) | med | Golden `.bin`/config-data vectors (6 ESCs) diff byte-for-byte; CRC has a known-answer test |
| Shared ABI drift (utypes ↔ objectlist ↔ ESI bit offsets) | med | Generate all three from **one** OD walk + one type table; CiA-402 golden vector exercises RECORD/ARRAY/PDO |
| Porting reference bugs by accident | med | [`bugs-ledger.md`](bugs-ledger.md) enumerates each with an explicit fix/keep decision |
| Float formatting divergence (REAL32/64 `float32ToHex`, decimal rendering) | med | Pin with new tests (reference has **none** for REAL); IEEE-754 bit patterns are unambiguous |
| String/number formatting drift (hex casing, `0x` vs `#x`, zero-pad) | low | Golden vectors catch it; formatting rules documented in research §Constants |
| Scope creep into unimplemented CoE features | low | Match reference limitations first; extend only on explicit request (brainstorming) |

## Effort estimate (rough)

Core generators + type model + I/O, TDD against existing vectors: on the order of a few
focused days. The long pole is the SII binary (`EEPROM.js`) and wiring up the golden-vector
harness; the four text generators are mostly string templating over one OD walk.

## Proposed Rust shape (single crate, lib-first)

`ponytail:` one crate, no premature module explosion — split only where the seams already
exist in the reference. CLI is a thin optional binary; the real product is the library
another project wraps.

```
src/
  model.rs        # Config (was `form`), Od/ObjectDict, Objd { Var|Array|Record }, Dc — serde JSON == esi.json
  types.rs        # Dtype enum + ESI_DT table (name/bitsize/ctype), Otype, Access — the C-ABI source of truth
  build.rs*       # build_object_dictionary: mandatory + sdo + pdo synthesis → flat od  (* name TBD, not build script)
  gen/
    objectlist.rs # objectlist.c
    utypes.rs     # utypes.h
    ecat_options.rs
    esi.rs        # ESI XML (with escaping — bug fix)
    eeprom.rs     # SII image: bytes, CRC-8, categories → Vec<u8>
    intel_hex.rs  # bin → Intel HEX + C header (was binaries.js)
  lib.rs          # generate(config, od, dc) -> GeneratedBundle { objectlist_c, utypes_h, ..., eeprom_bin }
tests/
  golden/         # lifted toEqualLines vectors: default, cia402, foe, per-dtype, 6 ESC config-data
```

### API sketch (the part another project wraps)

```rust
let project: Project = serde_json::from_str(esi_json)?;   // or built programmatically
let bundle = soes_generator::generate(&project)?;         // pure, infallible-ish
bundle.write_all(out_dir)?;                                // or take fields individually
// bundle: { objectlist_c: String, utypes_h: String, ecat_options_h: String,
//           esi_xml: String, eeprom_bin: Vec<u8>, eeprom_hex: String, eeprom_h: String }
```

Keep `generate` a pure `Project -> GeneratedBundle`; do file I/O only in `write_all` and
the CLI. Return `Result` for the validation the reference does with `alert()` (e.g. multiple
PDO mappings, bad `EEPROMsize`) — typed errors instead of silent drops.

### Dependencies (minimal)

- `serde` + `serde_json` — the JSON model. (Already the reference's real format.)
- Nothing else required for core. XML is emitted by templating (as the reference does),
  not a DOM lib — escaping is a 5-line helper. Add `thiserror` only if error variants grow.
- No zip crate needed for the library; the CLI can write loose files or add `zip` later.

## Test strategy

1. **Golden vectors first (TDD).** Lift the Jasmine `toEqualLines` expected strings into
   `tests/golden/*` and assert the Rust generators reproduce them byte-for-byte. Highest-value
   set: `emptyProjectSpecs` (default) + `cia402exampleProjectSpecs` (RECORD/ARRAY/PDO/DC) +
   `escSpecificSpecs` (6 config-data strings) + per-dtype VAR specs.
2. **Fill the reference's gaps** (unpinned in JS, so specify them in Rust): REAL32/REAL64
   values (bug 1), BOOLEAN PDO padding, non-multiple `EEPROMsize` (bug 5), validation.
3. **Differential oracle (optional, high confidence).** For fuzzed/random projects, run the
   JS reference via [`research/repro/harness.js`](research/repro/) and diff against Rust.
4. **CRC known-answer test** for the SII CRC-8 (poly 0x07 / init 0xFF / 14 bytes).

## Sequencing

1. `types.rs` + `model.rs` (+ serde round-trip of `esi.json` fixtures).
2. `build_object_dictionary` — validate against `odSpecs.js` structural vectors.
3. Text generators (`utypes` → `objectlist` → `ecat_options` → `esi`), each TDD'd to its
   golden string.
4. `eeprom.rs` + `intel_hex.rs` — the byte-exact SII, against the config-data + hex vectors.
5. Bundle + CLI + docs.

Fix the 5 bugs *as* each area is implemented (they cost nothing extra to do right the first
time and each lacks a reference test — write the missing test).

## Open questions → see brainstorming

Scope (which reference limitations to keep vs. extend), the public API shape another project
wants, whether ESI **import** is in scope, and error-handling philosophy. Raised interactively.
