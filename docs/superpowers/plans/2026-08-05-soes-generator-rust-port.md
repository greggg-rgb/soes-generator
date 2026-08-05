# SOES generator Rust port — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the JS EtherCAT EEPROM/SOES code generator to a Rust library that turns a `Project` (device config + object dictionary) into the SOES C files, ESI XML, and EEPROM binary/hex/header.

**Architecture:** One lib crate `soes_generator` + a thin `soes-gen` CLI. A pure core `generate(&Project) -> Result<Bundle, GenError>` (Strings + `Vec<u8>`, no I/O) underlies `emit()` (writes files, for build.rs) and the CLI. Faithful port validated byte-for-byte against golden vectors extracted from the JS reference's passing Jasmine suite; the 5 confirmed bugs are fixed with new tests.

**Tech Stack:** Rust 2024, `serde`/`serde_json`, `indexmap` (order-preserving OD maps), `thiserror`. Golden vectors dumped from the JS reference via a small Node/jsdom script. No `clap` (hand-rolled 2-arg CLI), no DOM/zip crates.

**Reference:** `.cache/EEPROM_generator/` (JS, commit `6f26c1b`, gitignored). Design spec: [`../specs/2026-08-05-soes-generator-rust-port-design.md`](../specs/2026-08-05-soes-generator-rust-port-design.md). Bug details: [`../../bugs-ledger.md`](../../bugs-ledger.md). Reference maps: [`../../research/`](../../research/).

## Global Constraints

- Rust edition **2024** (already in `Cargo.toml`).
- Dependencies limited to: `serde` + `serde_json`, `indexmap` (serde feature), `thiserror`. Nothing else. CLI args hand-rolled. XML via templating + a small escape helper.
- **Faithful port:** output must match the JS reference **byte-for-byte** on every golden vector, EXCEPT at the deliberately-fixed divergence points (bug 1 REAL32, bug 4 ESI ports, XML escaping) — those get their own pinned expected values.
- **Enum string values are the C ABI:** `DTYPE_UNSIGNED32`, `ATYPE_RO`, `OTYPE_VAR` macro suffixes are exactly the JS enum strings (`UNSIGNED32`, `RO`, `VAR`). Do not "tidy" them.
- Use **one numeric** bitsize table (`Dtype::esi().bitsize`, `u16`) everywhere — never the JS string-`'64'` form.
- Round-trip is **semantic equality** (load `esi.json` → equal model). Byte-exact `backup_json` re-serialization is not required by any golden vector.
- Keep reference limitations verbatim (single non-dynamic PDO/direction; `MAX_{RX,TX}PDO_SIZE = 512`; SM2/SM3 phys size `0`; `#endif __ESI_EEPROM_H__` bare token; ASCII/Latin-1 strings). Reject, don't silently mangle, at trust boundaries.

---

## File structure

```
Cargo.toml                       deps + [[bin]]
src/
  lib.rs           generate() / emit() / Bundle / Emitted / GenError; module wiring
  types.rs         Dtype (+ EsiType table), Otype, Access, PdoDir, Esc — the C-ABI source of truth
  names.rs         variableName + value/hex sanitizers (shared by od_build, utypes, objectlist)
  model.rs         Project / Config / OdSections / Objd / SubItem / SyncMode + serde + builder
  od_build.rs      build_object_dictionary: mandatory + values + SDO + TX/RX PDO synthesis → flat Od
  gen/mod.rs       re-exports the generators
  gen/utypes.rs    utypes.h
  gen/objectlist.rs objectlist.c   (+ REAL32 bug-1 fix, float32_to_hex)
  gen/ecat_options.rs ecat_options.h
  gen/esi.rs       ESI XML   (+ 4-port bug-4 fix, XML escaping)
  gen/eeprom.rs    SII image → Vec<u8> (word/byte layout, CRC-8, categories) + EEPROMsize validation
  gen/intel_hex.rs bin → Intel HEX + C header
  bin/soes_gen.rs  CLI
scripts/
  dump_golden.js   jsdom: run JS generators on each fixture, write tests/golden/**
tests/
  golden/<fixture>/{objectlist.c,utypes.h,ecat_options.h,device.xml,eeprom.bin,eeprom.hex,eeprom.h,configdata.txt}
  common/mod.rs    load_golden() + assert_eq_lines() helper (port of toEqualLines)
  *.rs             per-generator integration tests
```

Flat `Od` = `BTreeMap<u16, Objd>` (numeric key order == reference `getUsedIndexes`). Editable `OdMap` = `IndexMap<String, Objd>` (preserves JSON key order). `Objd` stores `pdo_mappings: Vec<String>` (serde-faithful to the reference); `Objd::pdo_dir() -> PdoDir` derives direction — this is the spec's "flat Objd carries PdoDir".

---

### Task 1: Crate scaffold, dependencies, module skeleton

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs` (replace the `add`/`it_works` stub)
- Create: `src/types.rs`, `src/names.rs`, `src/model.rs`, `src/od_build.rs`, `src/gen/mod.rs`, `src/gen/{utypes,objectlist,ecat_options,esi,eeprom,intel_hex}.rs` (empty `// TODO Task N` module files with a `pub` marker so the crate compiles)

**Interfaces:**
- Produces: a compiling empty crate with all modules declared; `GenError` enum stub; `pub fn generate` / `pub fn emit` signatures returning `todo!()`.

- [ ] **Step 1: Set dependencies in `Cargo.toml`**

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
indexmap = { version = "2", features = ["serde"] }
thiserror = "2"

[[bin]]
name = "soes-gen"
path = "src/bin/soes_gen.rs"
```

- [ ] **Step 2: Replace `src/lib.rs` with the module skeleton + error/API stubs**

```rust
pub mod types;
pub mod names;
pub mod model;
pub mod od_build;
pub mod gen;

use std::path::Path;
use model::Project;

#[derive(Debug, thiserror::Error)]
pub enum GenError {
    #[error("invalid config field {field}: {msg}")]
    Config { field: &'static str, msg: String },
    #[error("object dictionary error: {0}")]
    Od(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct Bundle {
    pub objectlist_c: String,
    pub utypes_h: String,
    pub ecat_options_h: String,
    pub esi_xml: String,
    pub eeprom_bin: Vec<u8>,
    pub eeprom_hex: String,
    pub eeprom_h: String,
    pub backup_json: String,
}

pub struct Emitted { pub paths: Vec<std::path::PathBuf> }

pub fn generate(_p: &Project) -> Result<Bundle, GenError> { todo!() }
pub fn emit(_p: &Project, _out_dir: &Path) -> Result<Emitted, GenError> { todo!() }
```

Create each `src/gen/*.rs` as `// implemented in a later task` and `src/gen/mod.rs` with `pub mod utypes; pub mod objectlist; pub mod ecat_options; pub mod esi; pub mod eeprom; pub mod intel_hex;`. Create `types.rs`/`names.rs`/`model.rs`/`od_build.rs` empty.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: builds with warnings about unused stubs; no errors.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml src/
git commit -m "scaffold: crate modules, deps, GenError/Bundle/API stubs"
```

---

### Task 2: `types.rs` — Dtype/EsiType table, Otype, Access, PdoDir, Esc

**Files:**
- Modify: `src/types.rs`
- Test: inline `#[cfg(test)] mod tests` in `src/types.rs`

**Interfaces:**
- Produces:
  - `enum Dtype { Boolean, Integer8, Integer16, Integer32, Integer64, Real32, Real64, Unsigned8, Unsigned16, Unsigned32, Unsigned64, VisibleString }`
  - `struct EsiType { pub iec_name: &'static str, pub bitsize: u16, pub ctype: &'static str }`
  - `impl Dtype { fn esi(&self) -> EsiType; fn macro_suffix(&self) -> &'static str; fn from_ident(s: &str) -> Option<Dtype> }` where `macro_suffix` returns the JS enum string (e.g. `"UNSIGNED32"`, `"VISIBLE_STRING"`) used to build `DTYPE_*` macros, and `from_ident` parses those same strings.
  - `enum Otype { Var, Array, Record }` with `fn macro_suffix(&self) -> &'static str` → `"VAR"`/`"ARRAY"`/`"RECORD"`.
  - `enum Access { Ro, Rw, Wo, RwPre }` with `fn from_str`/`fn macro_suffix` → `"RO"`/`"RW"`/`"WO"`/`"RWpre"` (note lowercase `pre`), default `Ro`.
  - `enum PdoDir { None, Tx, Rx }`
  - `enum Esc { Ax58100, Et1100, Lan9252, Lan9253Beckhoff, Lan9253Direct, Lan9253Indirect }` with `fn from_str(&str)->Option<Esc>` (accepts the reference strings incl. `"LAN9253 Beckhoff"`), `fn pdi_control(&self)->u8`, `fn reserved_0x05(&self)->u16`, `fn config_on_reserved_bytes(&self)->bool` (true for Ax58100 + all Lan9253), `fn max_mailbox_size(&self)->Option<u16>` (128 for Ax58100).

Ground truth for the table: `.cache/EEPROM_generator/src/constants.js` `ESI_DT` (lines 78-91), `DTYPE` (26-45), `SupportedESC`/`configOnReservedBytes` (159-174); ESC magic in `src/generators/EEPROM.js:33-57`.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn esi_table_matches_reference() {
        assert_eq!(Dtype::Unsigned32.esi(), EsiType { iec_name: "UDINT", bitsize: 32, ctype: "uint32_t" });
        assert_eq!(Dtype::Integer64.esi().bitsize, 64);              // numeric, not "64"
        assert_eq!(Dtype::Real32.esi(), EsiType { iec_name: "REAL", bitsize: 32, ctype: "float" });
        assert_eq!(Dtype::VisibleString.esi(), EsiType { iec_name: "STRING", bitsize: 8, ctype: "char" });
        assert_eq!(Dtype::Boolean.esi(), EsiType { iec_name: "BOOL", bitsize: 1, ctype: "uint8_t" });
    }
    #[test]
    fn macro_and_ident_roundtrip() {
        assert_eq!(Dtype::Unsigned32.macro_suffix(), "UNSIGNED32");
        assert_eq!(Dtype::from_ident("VISIBLE_STRING"), Some(Dtype::VisibleString));
        assert_eq!(Access::RwPre.macro_suffix(), "RWpre");
    }
    #[test]
    fn esc_config_bytes() {
        assert!(Esc::Ax58100.config_on_reserved_bytes());
        assert!(Esc::Lan9253Direct.config_on_reserved_bytes());
        assert!(!Esc::Et1100.config_on_reserved_bytes());
        assert_eq!(Esc::from_str("LAN9253 Beckhoff"), Some(Esc::Lan9253Beckhoff));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib types`
Expected: FAIL (types/methods not defined).

- [ ] **Step 3: Implement the enums + tables**

Add `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` on all. Implement `esi()` as a `match` returning the 12 `EsiType` rows from `constants.js:78-91`; `macro_suffix`/`from_ident` as matches over the DTYPE strings; `Esc` methods from `EEPROM.js:33-57`: `pdi_control` = `0x05` for default/AX58100/**LAN9253 Beckhoff**, `0x80` for LAN9252 **and LAN9253 Indirect**, `0x82` for **LAN9253 Direct only** (per-variant — do not collapse all LAN9253 to one value); `reserved_0x05` `0x0000` default, `0x001A` Ax58100, `0xC040` Lan9253 variants. Derive `EsiType` `PartialEq`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib types`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/types.rs
git commit -m "feat(types): Dtype/EsiType table, Otype, Access, PdoDir, Esc"
```

---

### Task 3: `names.rs` — variableName + value/hex sanitizers

**Files:**
- Modify: `src/names.rs`
- Test: inline tests in `src/names.rs`

**Interfaces:**
- Produces:
  - `fn variable_name(name: &str) -> String` — C identifier: remove chars `+ - * = ! @`, then replace `space . , ; : /` with `_`. (Ref `validation.js:95-109`.)
  - `fn sanitize_0x_hexa(s: &str) -> String` and parse helpers `fn parse_u32(field: &'static str, s: &str) -> Result<u32, GenError>` (accepts `0x`-prefixed or decimal, like JS `parseInt`). Used later for identity fields.

Ground truth: `validation.js`, `constants.js:221-222` (`charsToReplace`, `charsToRemove`).

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variable_name_sanitizes() {
        assert_eq!(variable_name("Device Type"), "Device_Type");
        assert_eq!(variable_name("Vendor-ID +x"), "VendorID_x");   // '-' and '+' removed, space -> '_'
        assert_eq!(variable_name("a.b,c;d:e/f"), "a_b_c_d_e_f");
    }
    #[test]
    fn parse_hex_or_dec() {
        assert_eq!(parse_u32("VendorID", "0x600").unwrap(), 0x600);
        assert_eq!(parse_u32("VendorID", "600").unwrap(), 600);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib names`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
use crate::GenError;
const REMOVE: &[char] = &['+', '-', '*', '=', '!', '@'];
const REPLACE: &[char] = &[' ', '.', ',', ';', ':', '/'];
pub fn variable_name(name: &str) -> String {
    name.trim().chars().filter(|c| !REMOVE.contains(c))   // .trim() matches sanitizeString (validation.js:95)
        .map(|c| if REPLACE.contains(&c) { '_' } else { c }).collect()
}
pub fn parse_u32(field: &'static str, s: &str) -> Result<u32, GenError> {
    let t = s.trim();
    let r = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(h, 16)
    } else { t.parse::<u32>() };
    r.map_err(|e| GenError::Config { field, msg: e.to_string() })
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib names`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/names.rs && git commit -m "feat(names): variable_name + hex/dec parse helpers"
```

---

### Task 4: `model.rs` — Project/Config/Objd + serde, round-trip the esi.json fixtures

**Files:**
- Modify: `src/model.rs`
- Create: `scripts/gen_fixtures.js` (emits the fixtures — see Step 1)
- Create test fixtures (generated): `tests/fixtures/{default,foe,cia402}.json`
- Test: `tests/model_roundtrip.rs`

**Interfaces:**
- Produces:
  - `type OdMap = indexmap::IndexMap<String, Objd>;`
  - `struct OdSections { pub sdo: OdMap, pub txpdo: OdMap, pub rxpdo: OdMap }`
  - `#[serde(tag = "otype")] enum Objd { #[serde(rename="VAR")] Var{..}, #[serde(rename="ARRAY")] Array{..}, #[serde(rename="RECORD")] Record{..} }` with fields: `dtype: Option<Dtype>` (via `#[serde(with=...)]` string form), `name: String`, `access: Option<Access>`, `value: Option<serde_json::Value>` (number-or-string, faithful), `size: Option<u16>`, `items: Vec<SubItem>`, `pdo_mappings: Vec<String>` (`#[serde(default)]`), `is_sdo_item: bool` (`#[serde(default)]` — set true for SDO-section objects in `od_build`; utypes' Parameters section keys off it, `utypes.js:47`, and it is NOT derivable from `pdo_mappings` since mandatory VARs also lack mappings).
  - `struct SubItem { name: String, dtype: Option<Dtype>, value: Option<serde_json::Value>, access: Option<Access> }` (`#[serde(default)]` on all but name).
  - `struct Config { .. ~31 fields, each `#[serde(rename="VendorID"/…)]`; the 7 CoE/FoE fields are `bool` with `#[serde(default)]` (the backup omits absent checkboxes — cia402 has no `DetailsEnableUseFoE`, so plain `bool` would fail deserialize); the rest `String`. }`
  - `struct SyncMode { name, description, assign_activate, sync0_cycle_time, sync0_shift_time, sync1_cycle_time, sync1_shift_time }` with renames matching backup keys (`Name`, `AssignActivate`, `Sync0cycleTime`, ...).
  - `struct Project { config: Config, #[serde(rename="od")] od: OdSections, dc: Vec<SyncMode> }` with `impl Project { pub fn from_json(s: &str) -> Result<Self, serde_json::Error>; pub fn from_json_file(p: &Path) -> ...; pub fn to_json(&self) -> String }`.
  - `impl Objd { pub fn pdo_dir(&self) -> PdoDir }` — `Tx` if `pdo_mappings` contains `"txpdo"`, `Rx` if `"rxpdo"`, else `None`.

Ground truth: `backup.js:33-51` (shape), `constants.js:183-218` (Config field names/defaults), research/01 §1-2, §6.

- [ ] **Step 1: Generate the fixtures with `scripts/gen_fixtures.js`**

The fixtures cannot be hand-copied: `getFormDefaultValues()` (`constants.js:183-218`) is a JS function returning an object with unquoted keys and `ESC: SupportedESC.ET1100` (an enum reference, not a string) — invalid JSON. So emit them from Node, where those symbols resolve. Write `scripts/gen_fixtures.js` that inlines `constants.js` + `cia402exampleProjectSpecs.js` (for `cia_esi_json`) via the same jsdom-free `require`/eval approach and writes:
- `tests/fixtures/cia402.json` = the `cia_esi_json` template string **verbatim** (already valid JSON).
- `tests/fixtures/default.json` = `JSON.stringify({ form: getFormDefaultValues().form, od: {sdo:{},txpdo:{},rxpdo:{}}, dc: [] }, null, 2)` (`ESC` resolves to `"ET1100"`).
- `tests/fixtures/foe.json` = same as default but `form.DetailsEnableUseFoE = true`.

Run: `cd .cache/EEPROM_generator && NODE_PATH="$(pwd)/node_modules" node ../../scripts/gen_fixtures.js`. Commit the three fixtures. The backup uses top-level key `form` → `#[serde(rename="form")] config` on `Project`. (Task 6's golden dump then consumes these same fixtures as input, guaranteeing fixture/golden fidelity from one source.)

- [ ] **Step 2: Write the failing test**

```rust
// tests/model_roundtrip.rs
use soes_generator::model::Project;
#[test]
fn cia402_roundtrips_semantically() {
    let src = std::fs::read_to_string("tests/fixtures/cia402.json").unwrap();
    let p = Project::from_json(&src).expect("deserialize");
    // spot-check load fidelity
    assert_eq!(p.config.vendor_id, "0x1337");   // cia402 fixture VendorID
    assert!(p.od.txpdo.len() + p.od.rxpdo.len() > 0);
    // semantic round-trip: reload our re-serialization, compare models
    let again = Project::from_json(&p.to_json()).unwrap();
    assert_eq!(p, again);   // derive PartialEq on the model
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --test model_roundtrip`
Expected: FAIL (types not defined / deserialize error).

- [ ] **Step 4: Implement the model**

Define the structs/enums with `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]`. Add a `mod dtype_str` serde helper that (de)serializes `Dtype` via `Dtype::from_ident`/`macro_suffix`. Implement `from_json`/`to_json`/`pdo_dir`. Field list for `Config` comes from `constants.js:183-218` — copy every control name as a `#[serde(rename)]`.

- [ ] **Step 5: Run tests**

Run: `cargo test --test model_roundtrip`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/model.rs tests/fixtures tests/model_roundtrip.rs
git commit -m "feat(model): Project/Config/Objd serde + esi.json round-trip"
```

---

### Task 5: Builder API

**Files:**
- Modify: `src/model.rs` (add builder)
- Test: `tests/builder.rs`

**Interfaces:**
- Produces: `impl Project { pub fn builder() -> ProjectBuilder }`; `ProjectBuilder` with `config: Config` public (set fields directly), and `add_sdo(Objd)`, `add_txpdo(Objd)`, `add_rxpdo(Objd)`, `build() -> Project`. Constructors `Objd::var(index, Dtype, name)`, `Objd::var_with_value(index, Dtype, name, value: &str)` (used by the REAL32/INTEGER64 tests), `Objd::array(...)`, `Objd::record(...)`. Index passed as `u16`, stored as the uppercase-hex key (`format!("{index:X}")`).

- [ ] **Step 1: Write the failing test**

```rust
// tests/builder.rs
use soes_generator::{model::Project, types::Dtype};
#[test]
fn builder_places_objects_by_section() {
    let p = Project::builder()
        .add_txpdo(soes_generator::model::Objd::var(0x6000, Dtype::Unsigned32, "input"))
        .build();
    assert!(p.od.txpdo.contains_key("6000"));
    assert_eq!(p.od.txpdo["6000"].pdo_dir(), soes_generator::types::PdoDir::Tx);
}
```

- [ ] **Step 2: Run to verify it fails** — Run: `cargo test --test builder` → FAIL.
- [ ] **Step 3: Implement** the builder + `Objd::var/array/record` (var sets `pdo_mappings` per section via the `add_*` method — `add_txpdo` pushes `"txpdo"`).
- [ ] **Step 4: Run tests** — Run: `cargo test --test builder` → PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(model): Project builder + Objd constructors"`

---

### Task 6: Golden-vector harness — dump JS reference output + Rust compare helper

**Files:**
- Create: `scripts/dump_golden.js`
- Consumes: `tests/fixtures/{default,foe,cia402}.json` (from Task 4)
- Create: `tests/golden/**` (generated, committed)
- Create: `tests/common/mod.rs`

**Interfaces:**
- Produces: committed golden files per fixture, and Rust helpers `common::load_golden(fixture, file) -> String` and `common::assert_eq_lines(actual, expected)` (line-by-line diff reporting the first differing line index — port of `spec/helpers/customMatchers.js` `toEqualLines`).

- [ ] **Step 1: Write `scripts/dump_golden.js`**

Reuse the jsdom loader from `docs/research/repro/harness.js` (inline all `src/*.js` + `spec/helpers/formMockHelper.js`). Drive it from the **three committed fixtures** (not the spec files — the per-dtype VAR inputs are closure-local in `spec/generators/VAR/*` and not extractable). For each of `default`/`foe`/`cia402`: read `tests/fixtures/<name>.json`, then `form = buildMockFormHelper(fixture.form)` (formMockHelper wraps the flat backup values into the `.value`/`.checked` mock the generators expect), `odSections = fixture.od`, `dc = fixture.dc`; build `od = buildObjectDictionary(form, odSections)`, `indexes = getUsedIndexes(od)`, then write:
`objectlist_generator`, `utypes_generator`, `ecat_options_generator`, `esi_generator(form,od,indexes,dc)`, `hex_generator(form)` (as `eeprom.bin`), `toIntelHex(...)` (`eeprom.hex`), `toEsiEepromH(...)` (`eeprom.h`), and `hex_generator(form,true)` (`configdata.txt`) — into `tests/golden/<fixture>/`. Also loop the 6 `SupportedESC` values (default form, override `form.ESC`) and dump `hex_generator(form,true)` into `tests/golden/esc/<esc>.txt`. (The INTEGER64 numeric-bitsize footgun is covered by a builder-constructed Rust unit test in Task 8, not a dumped VAR golden.)

- [ ] **Step 2: Run the dump and commit the goldens**

Run: `cd .cache/EEPROM_generator && npm install jsdom && NODE_PATH="$(pwd)/node_modules" node ../../scripts/dump_golden.js`
Expected: `tests/golden/**` populated. Eyeball `tests/golden/default/objectlist.c` for sanity (starts with `#include`).

- [ ] **Step 3: Patch the ESI goldens for the fixed divergence (bug 4)**

In every `tests/golden/*/device.xml`, replace `Physics="YY "` with `Physics="YY  "` (the 4-port fix output). Add a top-of-file note in `tests/common/mod.rs` documenting that these two chars are the only intentional divergence from the JS ESI output for the default-port fixtures. (No fixture contains `& < >`, so XML escaping doesn't alter any golden.)

- [ ] **Step 4: Write the Rust compare helper**

```rust
// tests/common/mod.rs
use std::path::Path;
pub fn load_golden(fixture: &str, file: &str) -> String {
    std::fs::read_to_string(Path::new("tests/golden").join(fixture).join(file))
        .unwrap_or_else(|e| panic!("missing golden {fixture}/{file}: {e}"))
}
pub fn assert_eq_lines(actual: &str, expected: &str) {
    for (i, (a, e)) in actual.lines().zip(expected.lines()).enumerate() {
        assert_eq!(a, e, "first diff at line {}", i + 1);
    }
    assert_eq!(actual.lines().count(), expected.lines().count(), "line count differs");
}
```

- [ ] **Step 5: Commit**

```bash
git add scripts/dump_golden.js tests/golden tests/common
git commit -m "test: golden vectors dumped from JS reference + line compare helper"
```

---

### Task 7: `od_build.rs` — build_object_dictionary

**Files:**
- Modify: `src/od_build.rs`
- Test: `tests/od_build.rs`

**Interfaces:**
- Consumes: `model::{Project, OdSections, Objd}`, `types::*`, `names::variable_name`.
- Produces: `pub type Od = std::collections::BTreeMap<u16, Objd>; pub fn build_object_dictionary(config: &Config, od: &OdSections) -> Result<Od, GenError>`. The flat `Od` merges: mandatory objects (`1000,1008,1009,100A,1018,1C00`) — `populateMandatoryObjectValues` (`od.js:267-280`) fills only `1008/1009/100A` (value+size from device/HW/SW strings) and `1018:01-04` (vendor/product/revision/serial) from `config`; `1000` stays `0x1389` and `1C00` items stay `1,2,3,4`; SDO items (with `data = &Obj.x` links via `variable_name`, and `is_sdo_item = true`); TX then RX PDO items (that order, `od.js:287-288`), synthesizing SM-assignment arrays (`1C13`/`1C12`) and per-object mapping records (`0x1Axx`/`0x14xx`), with BOOLEAN 7-bit padding. Each object's `pdo_mappings` set from its section.

Ground truth: `od.js:91-291` (the whole build), research/01 §3. This is the largest task — implement it in sub-steps mirroring the reference functions, committing after each green test.

- [ ] **Step 1: Write failing structural tests (mirror `odSpecs.js`)**

```rust
// tests/od_build.rs
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
    // 1000 stays constant 0x1389; 1018 is a RECORD with a Max SubIndex placeholder at items[0];
    // 1C00 array items are 1,2,3,4. Assert a representative few:
    // (exact accessors depend on the Objd API — assert 1000's value == 0x1389 and 1C00 has 4 real items)
}
#[test]
fn cia402_synthesizes_sm_assignment_and_mappings() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/cia402.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    assert!(od.contains_key(&0x1C12) || od.contains_key(&0x1C13)); // SM assignment synthesized
}
#[test]
fn validation_rejects_bad_input() {   // spec §Model validation set
    use soes_generator::{model::{Project, Objd}, types::Dtype};
    // PDO-mapped dtype not in dtypes_PDO_allowed (VISIBLE_STRING is allowed; use a disallowed case per constants.js:93)
    // duplicate object name across sections -> Err
    let p = Project::builder()
        .add_sdo(Objd::var(0x2000, Dtype::Unsigned32, "dup"))
        .add_txpdo(Objd::var(0x6000, Dtype::Unsigned32, "dup"))
        .build();
    assert!(build_object_dictionary(&p.config, &p.od).is_err(), "duplicate name must error");
}
```

- [ ] **Step 2: Run to verify it fails** — Run: `cargo test --test od_build` → FAIL.

- [ ] **Step 3: Implement incrementally** — port `getMandatoryObjects`, `populateMandatoryObjectValues`, `addSDOitems` (+ `objectlist_link_utypes` data links, set `is_sdo_item`), `addPdoObjectsSection` (SM assignment, mapping records, `getPdoMappingValue` bit-packing `0xINDEX SUBIDX BITSIZE`, boolean padding `booleanPaddingBitsize=7`). Set `pdo_mappings` per section. Keys stored as `u16`.

- [ ] **Step 3b: Implement the spec's validation set** (returns `GenError::Od`): object-name uniqueness across sections (`ui.js:434`), PDO-mapped dtype ∈ `dtypes_PDO_allowed` (`constants.js:93`, `ui.js:445`), and VISIBLE_STRING `size ≥ value.len()` (`ui.js:404`). (multiple-PDO-per-object and EEPROMsize are validated in Tasks 11/12; non-ASCII in Tasks 11/12 string paths.)

- [ ] **Step 4: Run tests** — Run: `cargo test --test od_build` → PASS.

- [ ] **Step 5: Commit** — `git commit -am "feat(od_build): build_object_dictionary with PDO/SM synthesis"`

---

### Task 8: `gen/utypes.rs` — utypes.h

**Files:**
- Modify: `src/gen/utypes.rs`
- Test: `tests/gen_utypes.rs`

**Interfaces:**
- Consumes: `od_build::Od`, `model::Config`, `types::*`, `names::variable_name`.
- Produces: `pub fn generate(config: &Config, od: &Od) -> String`.

Ground truth: `src/generators/utypes.js`, research/02 §5. `serial` uint32 always; Inputs = `pdo_dir()==Tx`, Outputs = `Rx`, Parameters = objects with `is_sdo_item` (NOT "no pdo_mappings" — mandatory VARs lack mappings too); ctype from `Dtype::esi().ctype`; VISIBLE_STRING → `char name[size]`; ARRAY → `ctype name[len]`; RECORD → anon struct.

- [ ] **Step 1: Write the failing golden test**

```rust
// tests/gen_utypes.rs
mod common;
use soes_generator::{model::Project, od_build::build_object_dictionary, gen::utypes};
#[test]
fn utypes_default_matches_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    common::assert_eq_lines(&utypes::generate(&p.config, &od), &common::load_golden("default", "utypes.h"));
}
#[test]
fn integer64_uses_numeric_bitsize_and_int64_ctype() {   // guards the JS string-'64' footgun
    use soes_generator::{model::Objd, types::Dtype};
    let p = soes_generator::model::Project::builder()
        .add_txpdo(Objd::var(0x6000, Dtype::Integer64, "big")).build();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    assert!(utypes::generate(&p.config, &od).contains("int64_t big;"));
}
```
Add the same golden test for `cia402`.

- [ ] **Step 2: Run to verify it fails** — Run: `cargo test --test gen_utypes` → FAIL.
- [ ] **Step 3: Implement** `generate` per the reference.
- [ ] **Step 4: Run tests** — PASS on default + cia402.
- [ ] **Step 5: Commit** — `git commit -am "feat(gen): utypes.h generator"`

---

### Task 9: `gen/objectlist.rs` — objectlist.c (+ REAL32 bug-1 fix)

**Files:**
- Modify: `src/gen/objectlist.rs`
- Test: `tests/gen_objectlist.rs`

**Interfaces:**
- Consumes: `Od`, `Config`, `types::*`, `names::variable_name`.
- Produces: `pub fn generate(config: &Config, od: &Od) -> String`; private `fn float32_to_hex(v: f32) -> String` (`format!("{:08X}", v.to_bits())`).

Ground truth: `src/generators/objectlist.js`. Per-otype `_objd` rows; flags `ATYPE_{access}` OR `ATYPE_{RXPDO|TXPDO}` from `pdo_dir()`; `SDOobjects[]` terminated `{0xffff,0xff,0xff,0xff,NULL,NULL}`; subindex zero-padded to 2 **decimal** digits then emitted after `0x` (reference quirk — subindex 10 → `0x10` meaning decimal 10, `objectlist.js:198-204`; NOT hex). **Bug-1 fix:** REAL32 default value encodes `float32_to_hex(objd.value)` (parse value to f32), NOT the placeholder. Copy the VISIBLE_STRING `'0'` value behavior deliberately (data travels in the `data` column).

- [ ] **Step 1: Write failing tests — golden + the new REAL32 test**

```rust
// tests/gen_objectlist.rs
mod common;
use soes_generator::{model::{Project, Objd}, od_build::build_object_dictionary, gen::objectlist, types::Dtype};
#[test]
fn objectlist_default_matches_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap()).unwrap();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    common::assert_eq_lines(&objectlist::generate(&p.config, &od), &common::load_golden("default", "objectlist.c"));
}
#[test]
fn real32_default_value_is_encoded_not_zeroed() {   // bug-1 regression (reference emits 0x00000000)
    let p = Project::builder().add_txpdo(Objd::var_with_value(0x6000, Dtype::Real32, "f", "1.5")).build();
    let od = build_object_dictionary(&p.config, &p.od).unwrap();
    let out = objectlist::generate(&p.config, &od);
    assert!(out.contains("0x3FC00000"), "REAL32 1.5 must encode IEEE-754, got:\n{out}");
}
```
(`Objd::var_with_value` is defined in Task 5.)

- [ ] **Step 2: Run to verify it fails** — Run: `cargo test --test gen_objectlist` → FAIL.
- [ ] **Step 3: Implement** `generate` + `float32_to_hex` with the bug-1 fix.
- [ ] **Step 4: Run tests** — golden (default+cia402) PASS, REAL32 test PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gen): objectlist.c generator (+REAL32 bug fix)"`

---

### Task 10: `gen/ecat_options.rs` — ecat_options.h

**Files:**
- Modify: `src/gen/ecat_options.rs`
- Test: `tests/gen_ecat_options.rs`

**Interfaces:**
- Consumes: `Od`, `Config`, `types::*`.
- Produces: `pub fn generate(config: &Config, od: &Od) -> String`.

Ground truth: `src/generators/ecat_options.js`. `#define`s for mailbox/SM addresses; `MAX_MAPPINGS_SM2/3` from `getMaxMappings` (count PDO-mapped subitems, **+1 per BOOLEAN**); `MAX_{RX,TX}PDO_SIZE = 512` kept hard-coded (reference limitation). Offsets via uppercase hex without `0x`.

- [ ] **Step 1: Write failing golden test** (default + cia402 + foe), same shape as Task 8.
- [ ] **Step 2: Run to verify it fails** — FAIL.
- [ ] **Step 3: Implement** `generate` + `get_max_mappings`.
- [ ] **Step 4: Run tests** — PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gen): ecat_options.h generator"`

---

### Task 11: `gen/esi.rs` — ESI XML (+ 4-port bug-4 fix + XML escaping)

**Files:**
- Modify: `src/gen/esi.rs`
- Create (partial): `src/gen/eeprom.rs` — `config_data_string` + a shared `write_config_area` (the rest of `eeprom.rs`, the full image + CRC, is Task 12)
- Test: `tests/gen_esi.rs`

**Interfaces:**
- Consumes: `Od`, `Config`, `model::SyncMode`, `types::*`.
- Produces: `pub fn generate(config: &Config, od: &Od, dc: &[SyncMode]) -> Result<String, GenError>` (embeds `<ConfigData>` via `config_data_string`); private `fn xml_escape(s: &str) -> String` (`& < > " '`).
  - Also produces (self-contained, needed here so esi has no forward dependency): `pub fn eeprom::config_data_string(config: &Config) -> Result<String, GenError>` and a private `fn write_config_area(buf: &mut [u8], config: &Config) -> Result<(), GenError>`. Config area = words 0-6 (bytes 0-13) only — the ConfigData string never needs the CRC (word 7) or categories (`EEPROM.js:349-357`), so it builds without the full image. Task 12's `hex_generator` reuses `write_config_area`.

Ground truth: `src/generators/esi_xml.js`, research/02 §2. **Bug-4 fix:** `Physics` concatenates all four `PortNPhysical` values (`P0+P1+P2+P3`), matching `getPhysicalPort`. **Escaping:** all user text/attribute values go through `xml_escape`. Multiple PDO mappings per object → `GenError` (not silent).

- [ ] **Step 1: Write failing tests — golden (patched) + the two bug tests**

```rust
// tests/gen_esi.rs
mod common;
#[test]
fn esi_default_matches_patched_golden() { /* assert_eq_lines vs tests/golden/default/device.xml (already "YY  ") */ }
#[test]
fn physics_includes_all_four_ports() {   // bug-4 regression
    // config with Port0..3 = Y,Y,Y,H  ->  Physics="YYYH"
}
#[test]
fn xml_text_is_escaped() {
    // device name with '&' -> "&amp;" appears in output, raw '&' does not
}
#[test]
fn config_data_string_matches_esc_golden() {
    // default config (ET1100) -> config_data_string == tests/golden/esc/et1100.txt (7 bytes hex)
}
```

- [ ] **Step 2: Run to verify it fails** — FAIL.
- [ ] **Step 3: Implement** `eeprom::write_config_area` + `eeprom::config_data_string` first (self-contained, no CRC), then esi `generate` + `xml_escape`, bug-4 4-port concat, multi-PDO `GenError`. Reject non-ASCII in ESI text fields (`GenError`, per Global Constraints).
- [ ] **Step 4: Run tests** — golden (default/cia402/foe) PASS, config-data + both bug tests PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gen): ESI XML + config_data_string (+4-port fix, +escaping)"`

---

### Task 12: `gen/eeprom.rs` — SII binary + CRC-8 + EEPROMsize validation

**Files:**
- Modify: `src/gen/eeprom.rs`
- Test: `tests/gen_eeprom.rs`

**Interfaces:**
- Consumes: `Config`, `types::Esc`, `names::parse_u32`, and `write_config_area` (from Task 11).
- Produces: `pub fn hex_generator(config: &Config) -> Result<Vec<u8>, GenError>` (full SII image; reuses `write_config_area` for words 0-6, then CRC + identity + mailbox + categories), private `fn find_crc(bytes: &[u8], n: usize) -> u8` (poly `0x07`, init `0xFF`, MSB-first). (`config_data_string`/`write_config_area` were implemented in Task 11.)

Ground truth: `src/generators/EEPROM.js`, research/02 §1, research/03 §1. LE word addressing (`word N` = byte `2N`); config words 0-7 (CRC at word 7 over first 14 bytes); identity words 8-15; mailbox words 24-28; word 62 = `floor(size/128)-1`; category area from byte `0x80` (STRING/GENERAL/FMMU/SYNCMANAGER). **Bug-5 fix:** validate `EEPROMsize` is a multiple of 32 (ideally 128) → `GenError` otherwise. Image pre-filled `0xFF`.

- [ ] **Step 1: Write failing tests — CRC known-answer + config-data goldens + full image + bug-5**

```rust
// tests/gen_eeprom.rs
mod common;
use soes_generator::{model::Project, gen::eeprom};
#[test]
fn crc8_known_answer() {
    // feed the default image's first 14 bytes; assert find_crc == the byte at word 7 of the golden bin
}
#[test]
fn configdata_matches_per_esc_goldens() {
    // for each of the 6 ESCs, set config.esc and assert config_data_string == tests/golden/esc/<esc>.txt
}
#[test]
fn full_image_matches_default_golden() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap()).unwrap();
    assert_eq!(eeprom::hex_generator(&p.config).unwrap(), std::fs::read("tests/golden/default/eeprom.bin").unwrap());
}
#[test]
fn eepromsize_not_multiple_of_32_errors() {   // bug-5 (reference panics)
    // config.eeprom_size = "2000" -> Err(GenError::Config{field:"EEPROMsize",..})
}
```

- [ ] **Step 2: Run to verify it fails** — FAIL.
- [ ] **Step 3: Implement** the full byte layout (reusing `write_config_area`), `find_crc`, EEPROMsize validation (bug-5), and reject non-ASCII in EEPROM string categories (`GenError`). Port helpers `writeEEPROMword_wordaddress` etc. as functions over `&mut [u8]`.
- [ ] **Step 4: Run tests** — all PASS. (This is the highest-risk task; if the full image differs, diff `xxd` of the golden vs output to locate the byte.)
- [ ] **Step 5: Commit** — `git commit -am "feat(gen): SII EEPROM binary + CRC-8 (+EEPROMsize validation)"`

---

### Task 13: `gen/intel_hex.rs` — Intel HEX + C header

**Files:**
- Modify: `src/gen/intel_hex.rs`
- Test: `tests/gen_intel_hex.rs`

**Interfaces:**
- Consumes: `&[u8]`.
- Produces: `pub fn to_intel_hex(record: &[u8]) -> Result<String, GenError>` and `pub fn to_esi_eeprom_h(record: &[u8]) -> String`.

Ground truth: `src/binaries.js`, research/03 §1c-d. Intel HEX: 32 bytes/record, big-endian 4-hex address, two's-complement checksum, `:00000001FF` EOF, uppercased. C header: 16 bytes/line `0xNN`, trailing `\n};\n#endif __ESI_EEPROM_H__` **verbatim**. Guard non-multiple-of-32 length (`GenError`) — the bug-5 root also lives here.

- [ ] **Step 1: Write failing tests** — golden `eeprom.hex` + `eeprom.h` for default; plus a `to_intel_hex(&[0u8; 20])` returns `Err` (not-multiple-of-32) test.
- [ ] **Step 2: Run to verify it fails** — FAIL.
- [ ] **Step 3: Implement** both, with the length guard.
- [ ] **Step 4: Run tests** — PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(gen): Intel HEX + C-array header"`

---

### Task 14: `lib.rs` — wire `generate()` + Bundle + GenError

**Files:**
- Modify: `src/lib.rs`
- Test: `tests/generate.rs`

**Interfaces:**
- Consumes: all generators + `od_build`.
- Produces: real `generate(&Project) -> Result<Bundle, GenError>` filling every field; `backup_json` = `project.to_json()`.

- [ ] **Step 1: Write the failing test**

```rust
// tests/generate.rs
mod common;
use soes_generator::{model::Project, generate};
#[test]
fn generate_default_fills_all_bundle_fields_from_goldens() {
    let p = Project::from_json(&std::fs::read_to_string("tests/fixtures/default.json").unwrap()).unwrap();
    let b = generate(&p).unwrap();
    common::assert_eq_lines(&b.objectlist_c, &common::load_golden("default", "objectlist.c"));
    common::assert_eq_lines(&b.utypes_h,     &common::load_golden("default", "utypes.h"));
    common::assert_eq_lines(&b.ecat_options_h,&common::load_golden("default", "ecat_options.h"));
    common::assert_eq_lines(&b.esi_xml,      &common::load_golden("default", "device.xml"));
    assert_eq!(b.eeprom_bin, std::fs::read("tests/golden/default/eeprom.bin").unwrap());
    common::assert_eq_lines(&b.eeprom_hex,   &common::load_golden("default", "eeprom.hex"));
    common::assert_eq_lines(&b.eeprom_h,     &common::load_golden("default", "eeprom.h"));
}
```

- [ ] **Step 2: Run to verify it fails** — FAIL (`todo!()`).
- [ ] **Step 3: Implement** `generate` by calling `build_object_dictionary` then each generator.
- [ ] **Step 4: Run tests** — PASS. Then run the FULL suite: `cargo test` → all green.
- [ ] **Step 5: Commit** — `git commit -am "feat: wire generate() over all generators"`

---

### Task 15: `emit()` + Emitted + output filenames

**Files:**
- Modify: `src/lib.rs`
- Test: `tests/emit.rs`

**Interfaces:**
- Produces: `emit(&Project, &Path) -> Result<Emitted, GenError>` writing `objectlist.c`, `utypes.h`, `ecat_options.h`, `eeprom.bin`, `eeprom.hex`, `eeprom.h`, `esi.json`, and `<variable_name(TextDeviceName)>.xml`. Returns the written paths.

- [ ] **Step 1: Write the failing test** — `emit` to a `std::env::temp_dir()` subdir, assert the 8 files exist and the `.xml` name equals `variable_name(config.text_device_name)` + `.xml`.
- [ ] **Step 2: Run to verify it fails** — FAIL.
- [ ] **Step 3: Implement** `emit` (call `generate`, write files).
- [ ] **Step 4: Run tests** — PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat: emit() writes the file bundle"`

---

### Task 16: `bin/soes_gen.rs` — CLI

**Files:**
- Modify: `src/bin/soes_gen.rs`
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `soes-gen <project.json> --out <dir>` — hand-rolled arg parse over `std::env::args`; loads `Project::from_json_file`, calls `emit`, prints written paths; non-zero exit + stderr on `GenError`.

- [ ] **Step 1: Write the failing test** — use `std::process::Command::new(env!("CARGO_BIN_EXE_soes-gen"))` on `tests/fixtures/default.json` into a temp dir; assert exit 0 and files present.
- [ ] **Step 2: Run to verify it fails** — FAIL.
- [ ] **Step 3: Implement** the CLI (≤30 lines).
- [ ] **Step 4: Run tests** — PASS.
- [ ] **Step 5: Commit** — `git commit -am "feat(cli): soes-gen <project.json> --out <dir>"`

---

### Task 17: build.rs usage docs + crate README

**Files:**
- Create: `README.md`
- Modify: `docs/README.md` (link the crate README)

**Interfaces:** none (docs only).

- [ ] **Step 1: Write `README.md`** — one runnable build.rs snippet (`soes_generator::emit(&Project::from_json_file("device.json".as_ref())?, &out_dir)?;` then feed `objectlist.c`/`*.h` to `cc`), the CLI usage, the `generate()` in-memory API, and a link to the design spec + bugs-ledger. State the kept reference limitations.
- [ ] **Step 2: Run the whole suite once more** — Run: `cargo test` → all green; `cargo build --release` → clean.
- [ ] **Step 3: Commit** — `git commit -am "docs: crate README with build.rs + CLI usage"`

---

## Self-review

**Spec coverage:** API (`generate`/`emit`/`Bundle`/`Emitted`/`GenError`) → Tasks 1,14,15. Model + serde (with `#[serde(default)]` on CoE/FoE bools) + builder → 4,5. `Dtype`/`ESI_DT`/one-numeric-bitsize → 2 (+ INTEGER64 guard test in 8). `PdoDir` on flat OD → 4 (`pdo_dir`), 7; `is_sdo_item` on flat OD → 4/7, consumed by utypes → 8. `od_build` → 7. Five generators → 8–13. Bug fixes: 1→T9, 4→T11(esi), 5→T12(eeprom)/T13, XML escaping→T11(esi); reader bugs 2,3 out of scope (spec). Validation set (dup-name / PDO-dtype / VISIBLE_STRING-size → T7 Step 3b; multiple-PDO → esi; EEPROMsize → eeprom/intel_hex; non-ASCII → esi + eeprom string paths). Golden vectors + oracle → 6 (driven by the fixtures from 4). CRC known-answer → eeprom task. Filenames → 15. build.rs/CLI → 15,16,17.

**Placeholder scan:** the empty `src/gen/*.rs` in Task 1 are deliberate compile stubs, each replaced by a named later task — not TBDs. Fixtures are generated by `scripts/gen_fixtures.js` (T4), not hand-copied. No "add error handling"/"write tests" hand-waves; every code step has real code or a named ground-truth function to port.

**Type consistency:** `generate`/`emit` signatures identical in Tasks 1/14/15; `build_object_dictionary(&Config,&OdSections)->Result<Od,_>` used consistently (7→8–13); text/ESI generators `(&Config,&Od[,&[SyncMode]])->…` , `eeprom::hex_generator(&Config)->Result<Vec<u8>>`, `intel_hex(&[u8])`. **`config_data_string`/`write_config_area` are implemented in Task 11 (esi) — self-contained, no CRC — and reused by Task 12 (eeprom); no forward dependency, no stub.** `Objd::var`/`var_with_value` constructors defined in Task 5 (used in 8/9). `is_sdo_item` field defined in Task 4, set in Task 7, read in Task 8. `indexmap` retained for the editable `OdMap` (preserves JSON key order incl. cia402's 1-digit `"A"` key; flat `Od` is `BTreeMap<u16,_>`).

> **Note on the esi↔eeprom dependency:** no task reorder was needed. ESI (Task 11) needs only the ConfigData string, so Task 11 implements `eeprom::config_data_string` + `write_config_area` self-contained (config words 0-6, no CRC); Task 12 (full EEPROM image) reuses `write_config_area`. Execution order 1→17 is dependency-correct. Bug-4 lives in the esi task (11), bug-5 in the eeprom (12) + intel_hex (13) tasks.
