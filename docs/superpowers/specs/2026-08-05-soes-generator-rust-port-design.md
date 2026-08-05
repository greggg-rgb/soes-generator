# SOES generator — Rust port design

**Date:** 2026-08-05
**Status:** approved (design), pending implementation plan
**Reference:** [kubabuda/EEPROM_generator](https://github.com/kubabuda/EEPROM_generator) @ `6f26c1b`, cloned at `.cache/EEPROM_generator` (gitignored)
**Background docs:** [`../../00-overview.md`](../../00-overview.md) · [`../../feasibility-and-port-plan.md`](../../feasibility-and-port-plan.md) · [`../../bugs-ledger.md`](../../bugs-ledger.md) · [`../../research/`](../../research/)

## Goal

Port the JavaScript EtherCAT EEPROM/SOES code generator to Rust so it integrates into a
Rust build workflow. Another project will wrap the SOES stack in ergonomic Rust APIs and
consume this generator's output (the SOES C files, ESI XML, and EEPROM image).

The generator is a **pure, deterministic transform**: `Project → { SOES C files, ESI XML,
EEPROM binary/hex/header }`. No async, no network, no runtime state.

## Decisions (locked)

| Decision | Choice |
|----------|--------|
| Integration model | **build.rs `emit()` + a CLI**, both over one pure in-memory core |
| Scope | **Faithful port + bug fixes**, model designed so dtypes / multi-PDO / ESI-import extend later without rework |
| Input format | **esi.json-compatible** (load existing web-tool projects) **+ a Rust builder** |
| ESI XML import | **Out of scope now** (JSON is the round-trip format); module boundary reserved for later |
| The 5 reference bugs | **Fixed**, each with the test the reference lacks; none break the golden vectors |

## Non-goals (v1)

- Multiple / dynamic PDOs per direction (reference limit; explicit typed error, not silent).
- CoE dtypes the reference doesn't implement (BIT1–8, BITARR, OCTET/UNICODE string, INTEGER24…).
- Parsing ESI XML back into the model.
- A GUI. A zip bundler in the library (CLI/build.rs write loose files; `cc` wants paths).
- Non-ASCII EEPROM strings (reference uses `charCodeAt` / Latin-1).

## Public API

```rust
// Pure core — no I/O. This is what everything else calls.
pub fn generate(p: &Project) -> Result<Bundle, GenError>;

// Convenience for build.rs / CLI: generate + write files to out_dir. Returns written paths.
pub fn emit(p: &Project, out_dir: &Path) -> Result<Emitted, GenError>;

pub struct Bundle {
    pub objectlist_c: String,      // SOES objectlist.c
    pub utypes_h: String,          // SOES utypes.h
    pub ecat_options_h: String,    // SOES ecat_options.h
    pub esi_xml: String,           // ESI device description (<project>.xml)
    pub eeprom_bin: Vec<u8>,       // raw SII image
    pub eeprom_hex: String,        // Intel HEX transcoding
    pub eeprom_h: String,          // C-array transcoding
    pub backup_json: String,       // esi.json project backup
}

pub struct Emitted { pub paths: Vec<PathBuf> }  // e.g. objectlist.c, utypes.h, ecat_options.h,
                                                //      <project>.xml, eeprom.{bin,hex,h}, esi.json
```

`emit` writes all artifacts; a build.rs typically then feeds `objectlist.c` + the two `.h`
files to the `cc` build, and treats the `.xml`/`.bin` as flashing/master artifacts. Callers
that want a subset use `generate` and write the fields they need.

**CLI:** `soes-gen <project.json> --out <dir>` — thin wrapper over `emit`.

**build.rs config source:** support both `Project::from_json_file(path)` (a `device.json`
checked into the wrapper crate, read at build time) and programmatic construction in build.rs.

## Model

`model.rs`, serde-serializable, mirroring the reference `esi.json` `{form, od, dc}` so
existing projects load unchanged. Field-level detail:
[`../../research/01-domain-model-and-flow.md`](../../research/01-domain-model-and-flow.md).

```rust
pub struct Project {
    pub config: Config,     // was `form` — ~31 device/ESC fields (identity, mailbox/SM
                            //   offsets, ports, ESC chip, SPI mode, 7 CoE/FoE flags)
    pub od: OdSections,     // editable OD, split; the runtime flat OD is derived, never stored
    pub dc: Vec<SyncMode>,  // Distributed-Clock op modes (ESI only)
}

pub struct OdSections { pub sdo: OdMap, pub txpdo: OdMap, pub rxpdo: OdMap }  // index(hex) → Objd

pub enum Objd {                                   // one variant per OTYPE
    Var   { dtype: Dtype, name, access, value, size: Option<u16> /* VISIBLE_STRING */ },
    Array { dtype: Dtype, name, access, items: Vec<SubItem> },      // subitems inherit dtype
    Record{ name, items: Vec<SubItem> },          // each subitem carries its own dtype
}
```

- `serde` for JSON. `Config` numeric fields kept as the raw strings the backup stores (hex or
  dec, `0x`-prefixed), parsed at point of use — matches the reference and preserves round-trip.
- **Builder** (`Project::builder()...`) for programmatic construction: `vendor_id`,
  `product_code`, `add_sdo`, `add_txpdo`, `add_rxpdo`, etc., with typed helpers
  (`Var::new(index, Dtype::U32, "name")`).
- Validation (reference's `alert()`-level rules) surfaces as `Result`/`GenError`, not silent
  drops: duplicate names, PDO-dtype restrictions, VISIBLE_STRING size ≥ value length,
  multiple PDO mappings per object, `EEPROMsize` not a multiple of 32/128.

## Modules

```
src/
  model.rs         Project / Config / OdSections / Objd / SubItem / SyncMode + serde + builder
  types.rs         Dtype enum + ONE ESI_DT table (name/bitsize/ctype); Otype; Access; ESC table
  od_build.rs      build_object_dictionary: mandatory + sdo + pdo synthesis → flat Od
  gen/
    objectlist.rs  objectlist.c   (_objd[] + SDOobjects[]; &Obj.x links)
    utypes.rs      utypes.h       (_Objects struct)
    ecat_options.rs ecat_options.h (#defines, mapping counts)
    esi.rs         ESI XML export (with XML escaping — bug fix)
    eeprom.rs      SII image → Vec<u8>: word/byte layout, CRC-8, categories
    intel_hex.rs   bin → Intel HEX + C header (was binaries.js)
  esi_import.rs    reserved boundary; `unimplemented!` in v1 (JSON is the round-trip today)
  lib.rs           generate() / emit() / Bundle / Emitted / GenError
  bin/soes_gen.rs  CLI
tests/
  golden/          lifted toEqualLines vectors: default, cia402, foe, per-dtype, 6 ESC config-data
```

Each `gen/*` module is one focused unit: input = `(&Config, &Od, &[Index])` (+ `&[SyncMode]`
for ESI), output = a `String` (or `Vec<u8>` for `eeprom`). `eeprom.rs` reads only `Config`.

## Extensibility seams (built as boundaries now, features later)

1. **`Dtype`** — enum backed by one `ESI_DT`-style table `(iec_name, bitsize, ctype)`. New CoE
   dtypes are table rows, not new match arms scattered across five generators. Uses **numeric**
   bitsizes uniformly, eliminating the reference's string-`'64'` footgun by construction.
2. **PDO synthesis** isolated in `build_object_dictionary`. The "single non-dynamic PDO per
   direction" limit is one explicit typed error; multi/dynamic PDO extends here without touching
   the generators.
3. **`esi_import`** module reserved; `unimplemented!` with a doc note pointing at
   [`research/03`](../../research/03-binaries-io-readers.md) §4 for the from-scratch OD parser.

## Code flow (mirrors the reference)

```
generate(project)
  ├─ build_object_dictionary(config, od)        // mandatory objs + values + SDO + TX/RX PDO synthesis
  │    → flat Od : map<hexIndex, Objd>          //   (SM-assign 1C12/1C13, 0x14xx/0x1Axx maps, BOOLEAN +7-bit pad)
  ├─ indexes = used_indexes(od)                 // sorted hex-string keys 0x1000..0xFFFF
  └─ each generator (config, od, indexes[, dc]) → Bundle field
```
Full walkthrough: [`../../00-overview.md`](../../00-overview.md) §Code flow.

## Bug fixes (from [bugs-ledger](../../bugs-ledger.md), each proven, each gets a new test)

| # | Fix |
|---|-----|
| 1 | REAL32 default encodes IEEE-754 of `objd.value` (`1.5`→`0x3FC00000`), not placeholder `0`. |
| 4 | ESI `Physics` includes all 4 ports; agrees with EEPROM `getPhysicalPort`. |
| 5 | Validate/pad `EEPROMsize` to a multiple of 32 (ideally 128); reject with a clear error otherwise. |
| — | ESI XML text/attributes are escaped (`& < >`). |
| 2, 3 | In the reader — **deferred** with ESI import (out of scope v1). |

Verified: bugs 1, 4 don't touch the default/CiA-402 golden paths (default ports `YY`, no REAL32
objects), so the fixes keep the reused vectors green. `#endif __ESI_EEPROM_H__` is reproduced
verbatim to keep the `binariesSpecs` vector green (latent, not "fixed").

## Testing

1. **Golden vectors first (TDD).** Lift the Jasmine `toEqualLines` expected strings into
   `tests/golden/*` — default (`emptyProjectSpecs`), CiA-402 (`cia402exampleProjectSpecs`,
   exercises RECORD/ARRAY/PDO/DC), FoE, per-dtype VAR, and the 6 ESC config-data strings —
   and assert each generator reproduces them byte-for-byte.
2. **Fill the reference's gaps** (unpinned in JS, specified here): REAL32/REAL64 values,
   BOOLEAN PDO padding, non-multiple `EEPROMsize`, builder validation.
3. **CRC known-answer test** for the SII CRC-8 (poly `0x07`, init `0xFF`, 14 bytes).
4. **Differential oracle (optional).** For fuzzed projects, run the JS reference via
   [`../../research/repro/harness.js`](../../research/repro/) and diff against Rust output.

## Dependencies (minimal)

- `serde` + `serde_json` — the model / esi.json.
- `thiserror` — `GenError` variants (only if they grow; else a hand-written enum).
- CLI: `clap` (small) or hand-rolled arg parse. Nothing else. XML is templated + a 5-line
  escape helper (no DOM crate); no zip crate in the library.

## Build order

1. `types.rs` + `model.rs` (+ serde round-trip of `esi.json` fixtures) + builder.
2. `build_object_dictionary` — validate against `odSpecs.js` structural vectors.
3. Text generators `utypes` → `objectlist` → `ecat_options` → `esi`, each TDD'd to its golden string.
4. `eeprom.rs` + `intel_hex.rs` — byte-exact SII, against config-data + hex vectors.
5. `generate` / `emit` / CLI, then docs.

Fix each bug as its area is implemented (costs nothing extra done right the first time; each
lacks a reference test — write the missing test then).
