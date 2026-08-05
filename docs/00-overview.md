# SOES generator — architecture overview

A Rust port of [kubabuda/EEPROM_generator](https://github.com/kubabuda/EEPROM_generator),
a code generator for EtherCAT slave devices built on the
[SOES](https://github.com/OpenEtherCATsociety/SOES) stack. Reference cloned at
`.cache/EEPROM_generator` (commit `6f26c1b`, gitignored).

## What it is

**One pure function**: `(device config + object dictionary) → a bundle of consistent
output files`. No network, no async, no runtime state, no UI logic in the core. The
browser reference wraps it in a form + localStorage; the port keeps only the generator
core and swaps the browser shell for a Rust API + plain file writes.

## Inputs

1. **`form`** — ~31 device/ESC config fields (identity, mailbox/SM offsets, ports, ESC
   chip, SPI mode, 7 CoE/FoE flags). Full table:
   [`research/01-domain-model-and-flow.md`](research/01-domain-model-and-flow.md) §2.
2. **`odSections`** — the editable Object Dictionary, split `{ sdo, txpdo, rxpdo }`,
   each a map of `hexIndex → objd`. An `objd` is a VAR / ARRAY / RECORD with `dtype`,
   `name`, `access`, `value`, `items`, `pdo_mappings`. §1.
3. **`dc`** — optional Distributed-Clock op-mode list (ESI only).

These three are exactly the JSON backup shape (`backup.js`) — the lossless round-trip
format. The flat runtime `od` is **derived**, never stored.

## Outputs (7 artifacts + backup)

| File | Generator | Consumer |
|------|-----------|----------|
| `objectlist.c` | `objectlist.js` | SOES slave stack (the OD `_objd[]` / `SDOobjects[]`) |
| `utypes.h` | `utypes.js` | SOES — `_Objects` struct backing the `&Obj.x` pointers |
| `ecat_options.h` | `ecat_options.js` | SOES — mailbox/SM `#define`s, mapping counts |
| `<project>.xml` (ESI) | `esi_xml.js` | EtherCAT master (TwinCAT etc.) device description |
| `eeprom.bin` | `EEPROM.js` `hex_generator` | raw SII image flashed to the ESC EEPROM |
| `eeprom.hex` | `binaries.js` `toIntelHex` | Intel-HEX transcoding of the image |
| `eeprom.h` | `binaries.js` `toEsiEepromH` | C-array transcoding of the image |
| `esi.json` | `backup.js` | project save/restore (authoritative round-trip) |

**Consistency is the whole point**: struct field order in `utypes.h`, `&Obj.x` links in
`objectlist.c`, and PDO bit offsets in the ESI must all agree, or the slave won't parse.

## Code flow

```
processForm(form)                                    ui.js:113
  │
  ├─ buildObjectDictionary(form, odSections)         od.js:282  ── the key transform
  │     ├─ getMandatoryObjects()                     1000,1008,1009,100A,1018,1C00
  │     ├─ populateMandatoryObjectValues(form)       identity/versions from form
  │     ├─ addSDOitems()                             + objectlist_link_utypes → .data = &Obj.x
  │     ├─ addTXPDOitems()  ┐ synthesize 1C13 SM-assign array
  │     └─ addRXPDOitems()  ┘ synthesize 1C12 + per-object 0x14xx/0x1Axx mapping records
  │                          (BOOLEAN → +7-bit padding entry)
  │   ⇒ flat `od` : map<hexIndex, objd>
  │
  ├─ getUsedIndexes(od)                              sorted hex-string keys 0x1000..0xFFFF
  │
  └─ generators, each (form, od, indexes[, dc]):
        objectlist_generator · ecat_options_generator · utypes_generator
        hex_generator(form) → toIntelHex / toEsiEepromH
        esi_generator(form, od, indexes, dc)
        prepareBackupFileContent(form, odSections, dc)   ⇒ esi.json
```

`hex_generator` (the SII/EEPROM binary) is the odd one out — it reads **only `form`**,
not the OD. The OD lives in the ESI XML and the C files, not the binary image.

## The hard parts (where a port earns its keep)

Ranked; details in [`research/02-generators.md`](research/02-generators.md):

1. **SII binary byte/word math** (`EEPROM.js`) — LE word addressing (`offset/2`), the
   CRC-8 over 14 bytes (poly `0x07`, init `0xFF`, MSB-first), STRING category parity
   padding, per-ESC magic register values, `EEPROMsize = floor(bytes/128)-1`. One wrong
   bit and the ESC rejects the image.
2. **Shared implicit ABI** across `utypes` / `objectlist` / ESI — the DTYPE/ATYPE/OTYPE
   **enum string values are C macro names**; struct field order and PDO bit offsets must
   match exactly.
3. **PDO synthesis** (`addPdoObjectsSection`) — mapping-value bit packing
   `0xINDEX SUBIDX BITSIZE`, BOOLEAN 7-bit padding counted in three places.
4. **Type tables** — port `ESI_DT` (name/bitsize/ctype) as the single source of truth;
   avoid the reference's string-`'64'` size footgun.

## Bugs

5 confirmed (with proofs), plus latent issues — see [`bugs-ledger.md`](bugs-ledger.md).
The reference **73-spec Jasmine suite passes**; its golden `toEqualLines` strings are the
port's regression oracle ([`research/04-tests-and-bugs.md`](research/04-tests-and-bugs.md)).

## Reference limitations (inherited unless we decide otherwise)

- Single, non-dynamic PDO per direction (TX / RX).
- Some CoE data types unimplemented (BIT1–8, BITARR, OCTET/UNICODE string, INTEGER24…).
- ESI XML is write-mostly; re-import is a stub.
- ASCII/Latin-1 only in EEPROM strings (`charCodeAt`).
