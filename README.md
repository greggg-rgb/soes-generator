# soes_generator

Rust port of the [EEPROM_generator](https://github.com/kubabuda/EEPROM_generator) tool: turns a
device JSON project file (vendor/product identity, object dictionary, PDO mappings, DC sync
modes) into the C source, ESI XML, and EEPROM binary a [SOES](https://github.com/OpenEtherCATsociety/SOES)
EtherCAT slave needs.

## Build-time codegen (`build.rs`)

Call `soes_generator::emit` from a `build.rs` to generate the SOES source files into `OUT_DIR`
at compile time:

```rust
// build.rs
fn main() {
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let project = soes_generator::model::Project::from_json_file("device.json".as_ref())
        .expect("load device.json");
    soes_generator::emit(&project, &out).expect("emit SOES files");
    println!("cargo:rerun-if-changed=device.json");
}
```

`emit` writes eight files into the output directory: `objectlist.c`, `utypes.h`,
`ecat_options.h`, `eeprom.bin`, `eeprom.hex`, `eeprom.h`, `esi.json` (the JSON backup, for
round-tripping the project), and `<device-name>.xml` (the ESI file). Feed `objectlist.c`
together with `utypes.h`/`ecat_options.h`/`eeprom.h` to your C compiler (e.g. via the [`cc`
crate](https://docs.rs/cc)) alongside the SOES stack sources — they compile straight into it,
no further code generation step needed.

## CLI usage

```
soes-gen <project.json> --out <dir>
```

Reads a project JSON file and writes the same eight files listed above into `<dir>`, printing
each output path.

## In-memory API

For callers that want the generated artifacts without touching the filesystem, use
`generate`:

```rust
let project = soes_generator::model::Project::from_json_file("device.json".as_ref())?;
let bundle = soes_generator::generate(&project)?;
```

`generate` returns a `Bundle`:

| Field | Type | Contents |
|---|---|---|
| `objectlist_c` | `String` | CoE object dictionary (`objectlist.c`) |
| `utypes_h` | `String` | PDO struct typedefs (`utypes.h`) |
| `ecat_options_h` | `String` | mailbox/PDO size options (`ecat_options.h`) |
| `esi_xml` | `String` | ESI XML device description |
| `eeprom_bin` | `Vec<u8>` | raw SII EEPROM image |
| `eeprom_hex` | `String` | Intel HEX encoding of `eeprom_bin` |
| `eeprom_h` | `String` | EEPROM image as a C header (`eeprom.h`) |
| `backup_json` | `String` | the project re-serialized to JSON (round-trip format) |

`emit(&project, &out_dir)` calls `generate` and writes the `Bundle` to disk, returning an
`Emitted { paths: Vec<PathBuf> }` of the files written. Both functions return
`Result<_, GenError>` (`GenError::Config`, `GenError::Od`, `GenError::Io`).

Projects can also be built in code instead of loaded from JSON, via
`soes_generator::model::Project::builder()` (seeded with the reference tool's form defaults)
and its `add_sdo`/`add_txpdo`/`add_rxpdo` methods, together with the `Objd::var` /
`var_with_value` / `var_string` / `array` / `record` constructors.

## Docs

- [Design spec](docs/superpowers/specs/2026-08-05-soes-generator-rust-port-design.md) — full
  architecture and module layout.
- [Bugs ledger](docs/bugs-ledger.md) — every bug found in the JS reference and the port
  decision for each.

## Kept reference limitations

This port intentionally matches the reference tool's constraints rather than generalizing
beyond it:

- **Single non-dynamic PDO per direction.** Multiple PDO mappings per object in the same
  direction (Tx or Rx) is a reference limitation, not supported.
- **`MAX_RXPDO_SIZE`/`MAX_TXPDO_SIZE` are hard-coded at 512** in `ecat_options.h`, regardless
  of actual PDO content size.
- **SM2/SM3 physical size is written as `0`** in the EEPROM SyncManager category; it is never
  validated against actual PDO length.
- **`eeprom.h` ends with `#endif __ESI_EEPROM_H__`** — a bare token after `#endif` (not valid
  ISO C under `-pedantic`), byte-for-byte matched to the reference tool's output.
- **EEPROM strings are ASCII/Latin-1**, not UTF-8.

## Intentional divergences from the reference tool

A few reference bugs are fixed rather than reproduced (see the [bugs ledger](docs/bugs-ledger.md)
for proofs and full detail):

- **REAL32 default values** are now encoded from the object's actual `value` (IEEE-754), not
  emitted as `0x00000000` for every REAL32.
- **ESI `Physics` attribute reflects all 4 ports**, not just the first 3 (the reference's
  operator-precedence bug dropped Port3).
- **XML text/attribute values are escaped** (`& < >`), which the reference's manual XML builder
  did not do.
