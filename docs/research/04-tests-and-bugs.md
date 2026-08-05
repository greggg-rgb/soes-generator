# 04 — Test Suite Map & Bug Hunt (EEPROM_generator JS reference)

Scope: the Jasmine test suite under `spec/` and a bug hunt across the writers
(`EEPROM.js`, `esi_xml.js`, `binaries.js`) vs. the reader (`xml_reader.js`).
Reference tree: `.cache/EEPROM_generator/`.

**All findings below were verified by running the suite (and custom probes)
headlessly** — see "Running headless" for the harness. The 5 confirmed bugs
each have a runnable input → expected → actual.

---

## Part A — Test Map

### Running headless (proven)

The suite is browser-based (`tests.html`, Jasmine 3.8, loaded as `<script>`
tags). It **does run under Node** with `jsdom` — no source changes needed:

- Almost all browser globals (`alert`, `document.createElement`,
  `localStorage`) are only touched inside error paths or unexercised code, so
  the generator specs never hit them.
- The reader (`xml_reader.js`) needs `DOMParser`, which `jsdom` provides.
- Harness approach that works: build one HTML string that inlines
  `lib/jasmine-3.8.0/{jasmine,jasmine-html,boot}.js` + all `src/` files + all
  `spec/` files as bare `<script>` blocks (do **not** wrap them in
  `try{}` — that block-scopes the top-level `const`s and every cross-file
  global disappears), load it in `new JSDOM(html, {runScripts:'dangerously'})`,
  add a reporter capturing `specDone`/`jasmineDone`, read results after load.
- Result: **73 specs, all pass, overall `passed`.** (`spec/generators/VAR/empty_Specs.js`
  exists but is *not* referenced by `tests.html`, so its 5 specs never run.)

This gives the Rust port a live oracle: feed any form/OD into the JS generators
and diff against the Rust output.

### What is covered, and how thoroughly

| Spec file | Target(s) | Depth |
|---|---|---|
| `generators/emptyProjectSpecs.js` (1421 L) | `esi_generator`, `hex_generator` (config data + full Intel-hex), `ecat_options_generator`, `objectlist_generator`, `utypes_generator` for the **default form**, both fresh and restored-from-localStorage | Full golden-string match of every generator. The single most valuable vector set. |
| `generators/cia402exampleProjectSpecs.js` (1697 L) | Same 5 generators for a **populated CiA-402 servo** project (restored from `cia_esi_json` backup) | Full golden-string match. Exercises RECORDs, ARRAYs, TxPDO/RxPDO, SM assignments, DC. |
| `generators/enabledFoEProjectSpecs.js` (554 L) | Same 5 generators **+ Intel-hex** with `DetailsEnableUseFoE` on | Golden match. Pins the FoE protocol bit (0x0C) and bootstrap-mailbox words. |
| `generators/escSpecificSpecs.js` (127 L) | `hex_generator` **config-data string** for ET1100, AX58100, LAN9252, LAN9253 Beckhoff/Direct/Indirect | 6 golden config-data strings. Pins `pdiControl`/`reserved_0x05` per ESC and the 7-vs-14-byte config length. |
| `generators/spiModeHexSpecs.js` (33 L) | `hex_generator` config data across SPImode values | 1 spec. |
| `generators/VAR/{INTEGER8,INTEGER64,VISIBLE_STRING,ARRAY}_Specs.js` | All 5 generators with a single VAR/ARRAY of that dtype added as TXPDO (+ VISIBLE_STRING also as SDO) | Golden match per generator. Pins PDO mapping value, bitsize, utypes ctype, objectlist DTYPE. |
| `generators/VAR/empty_Specs.js` | (same shape) | **Present but not wired into `tests.html` — does not run.** |
| `odSpecs.js` (214 L) | `buildObjectDictionary` (empty, +SDO, +TxPDO, +RxPDO, full CiA-402), `setArrayLength` | Object-graph `toEqual`. Good structural coverage of OD building. |
| `binariesSpecs.js` (29 L) | `toEsiEepromH` (eeprom.h C array) | 1 golden vector. |
| `readers/xml_reader_cia402exampleSpecs.js` (1363 L) | `xml_reader` on `CiA402_example.xml` | **Shallow**: ~11 scalar `expect`s (Vendor, IDs, ProfileNo, EEPROMsize, group/device names, ESC, SPImode). **All CoE-detail, port, mailbox and the whole-object JSON assertions are commented out.** OD reading is a `TODO` and untested. |
| `backupSpecs.js` (13 L) | `isFormControlAButton` | 1 trivial spec. |

### Fixtures / helpers (reusable golden vectors)

- **`spec/helpers/formMockHelper.js`** — `buildMockFormHelper(formValues?)`
  builds a fake HTML-form object: each control is `{ name, value }` (or
  `{ name, checked }` for checkboxes), driven by `getFormDefaultValues().form`
  in `src/constants.js`. `getForm()` returns the last-built mock; `getEmptyFrom()`
  returns a bare form of empty controls (used by restore tests). This is the
  entire "form" contract the generators consume — the Rust port's form struct
  can mirror `getFormDefaultValues()` field-for-field.
- **`spec/helpers/customMatchers.js`** — `toEqualLines`: line-by-line string
  compare that reports the first differing line index. Every generator spec
  asserts a full expected multi-line string with `toEqualLines`. **These
  expected strings are ready-made golden output vectors for the port** — the
  default project, CiA-402 project, FoE project, per-dtype VAR projects, and
  the 6 ESC config-data strings can be lifted verbatim as Rust integration
  fixtures.
- Backup fixtures: `cia_esi_json` / `cia_esi_json`-style JSON blobs embedded in
  the CiA-402 and od specs feed `restoreBackup(...)` to reconstruct a populated
  project without the UI.

### Coverage GAPS (behavior left unpinned — port cannot rely on tests here)

1. **Reader is barely tested.** `xml_reader` asserts ~11 scalars; CoE details,
   ports, mailbox offsets, and *all OD parsing* are commented out or `TODO`.
   Writer→reader round-trip is **not** tested at all. (Two confirmed reader
   bugs below live in exactly this untested region.)
2. **REAL32 / REAL64 have no specs** anywhere. (Confirmed float bug below is
   therefore unpinned.)
3. **BOOLEAN** dtype: padding logic (`pdoBooleanPadding`,
   `addBooleanPadding`, the `getMaxMappings` boolean double-count) is never
   exercised by a spec.
4. **RECORD utypes** generation is marked `/* TODO test */` and only indirectly
   hit via the CiA-402 fixture.
5. **Input validation / robustness**: no spec feeds an invalid `EEPROMsize`,
   non-hex offset, over-long string, or a dtype the tables don't know. The
   `validation.js` sanitizers (`sanitizeInt/Uint/Float/Bool`, `variableName`)
   have **zero** direct specs.
6. **Intel-hex** only tested at 2048 bytes (a multiple of 32). Non-multiples are
   untested — and crash (bug #5).
7. **Multiple PDO mappings, arrays of booleans, OCTET/UNICODE strings,
   INTEGER24/UNSIGNED24** — explicitly unimplemented and untested.
8. `empty_Specs.js` is dead (not loaded).

---

## Part B — Bugs

Ranked by severity. "CONFIRMED" = reproduced with a runnable input via the
jsdom harness (probe scripts drove the real `src/` functions).

### BUG 1 — REAL32 default value is always emitted as `0x00000000` in objectlist.c  (CONFIRMED, medium-high)

`src/generators/objectlist.js:183-196`, `objectlist_getItemValue`:

```js
function objectlist_getItemValue(objd, dtype) {
    let value = '0';
    if (objd.value) {
        switch(dtype) {
            case DTYPE.REAL32:
                return `0x${float32ToHex(value)}`;   // <-- uses local `value` ('0'), not objd.value
            case DTYPE.VISIBLE_STRING:
                return value;
            default:
                return `${objd.value}`;
        }
    }
    return value;
}
```

`float32ToHex` is called with the literal placeholder `value = '0'`, never
`objd.value`. So any REAL32 object's default value is silently discarded and
encoded as float 0.

- **Input:** VAR `{ otype:'VAR', dtype:'REAL32', name:'MyFloat', value:1.5 }`,
  run `objectlist_generator`.
- **Expected:** value column = IEEE-754 of 1.5 = `0x3FC00000`.
- **Actual (proven):** `  {0x0, DTYPE_REAL32, 32, ATYPE_RO, acName2000, 0x00000000, NULL},`

Root cause is one token: `float32ToHex(value)` should be
`float32ToHex(objd.value)`. Unpinned (no REAL32 spec). For the Rust port: the
REAL32 encoding path is *specified nowhere correctly* — implement it right and
add the missing test.

### BUG 2 — Reader writes CoE "Upload at startup" to the wrong field → round-trip data loss  (CONFIRMED, medium)

`src/readers/xml_reader.js:90`:

```js
result.form.CoeDetailsEnablePdoUploadAtStartup = DeviceMailboxCoE.attributes['PdoUpload'].value == 'true';
```

The rest of the app (default form `constants.js:214`, writer
`EEPROM.js:276` → CoE-details bit `0x10`, ESI writer `esi_xml.js:345`) uses the
field name **`CoeDetailsEnableUploadAtStartup`**. The reader sets a *different*
key `CoeDetailsEnable**Pdo**UploadAtStartup`, so on ESI→form restore the real
flag stays `undefined` and the 0x10 CoE-details bit is lost.

- **Input:** ESI with `<CoE ... PdoUpload="true" ...>`, run `xml_reader`.
- **Expected:** `result.form.CoeDetailsEnableUploadAtStartup === true`.
- **Actual (proven):** `CoeDetailsEnableUploadAtStartup` is `undefined`;
  `CoeDetailsEnablePdoUploadAtStartup === true` (value landed on the dead key).

Not caught because the reader spec's CoE assertions are commented out.

### BUG 3 — Reader throws on a device with an empty `<Type>` (typo'd DOM method)  (CONFIRMED, medium)

`src/readers/xml_reader.js:40-42`:

```js
if (!result.form.TextDeviceType) {
    result.form.TextDeviceType = Device.getElementsByTagGroupType('GroupType')[0].innerHTML;
}
```

`getElementsByTagGroupType` is not a DOM method (typo for
`getElementsByTagName`). It is dormant only because the fallback runs solely
when `<Type>` innerHTML is empty — which the one test fixture never is.

- **Input:** same ESI but `<Type ProductCode="#xab" RevisionNo="#x2"></Type>`
  (empty), run `xml_reader`.
- **Expected:** device type falls back to `<GroupType>` text.
- **Actual (proven):** `TypeError: Device.getElementsByTagGroupType is not a function` — the whole restore aborts.

### BUG 4 — ESI `Physics` attribute drops Port3, and disagrees with the EEPROM  (CONFIRMED, medium)

`src/generators/esi_xml.js:26`:

```js
`<Device Physics="${form.Port0Physical.value + form.Port1Physical.value + form.Port2Physical.value || + form.Port3Physical.value}">`
```

Operator precedence makes this `((P0+P1+P2) || (+P3))`. `P0+P1+P2` is a
non-empty string (default `"YY "`), always truthy, so the `|| (+P3)` branch is
never taken and **Port3 is never included** in the ESI `Physics` string. (The
`+P3` unary-plus is also nonsense — would be `NaN` for `"H"`.) Meanwhile
`EEPROM.js:281-301 getPhysicalPort` reads **all four** ports into the EEPROM
physical-port word. So ESI and EEPROM disagree whenever port 3 is used.

- **Input:** ports P0=`Y`, P1=`Y`, P2=`Y`, P3=`H`, run `esi_generator`.
- **Expected:** `Physics="YYYH"` (or the tool's 4-char encoding incl. port 3).
- **Actual (proven):** `Physics="YYY"` — port 3 silently dropped.

### BUG 5 — `toIntelHex` crashes when `EEPROMsize` is not a multiple of 32  (CONFIRMED, low-medium)

`src/binaries.js:17-31`. `rulesTotalCount = record.length/bytes_per_rule`
(32). The loop condition `rulenumber < rulesTotalCount` runs a partial final
rule when the size isn't a multiple of 32; `CreateiHexRule` then reads
`record[byteposition]` past the array end and calls `.toString(16)` on
`undefined`.

`EEPROMsize` is a free-text `<input>` (`index.html:221`), so any value is
possible, and there is no validation that it's a multiple of 32 (EEPROM.js even
assumes multiples of 128: `(EEPROMsize/128)-1`).

- **Input:** default form with `EEPROMsize = "2000"`, `toIntelHex(hex_generator(form))`.
- **Expected:** valid Intel-hex (or a validation error).
- **Actual (proven):** `TypeError: Cannot read properties of undefined (reading 'toString')`.
  (`EEPROMsize = "2048"` works.)

For the port: pad/validate `EEPROMsize` to a multiple of the record width (and
ideally 128) before emitting Intel-hex.

### Lower-severity / latent (worth pinning in the port, not all "bugs")

- **`dtype_bitsize` stores 64-bit sizes as the *string* `'64'`**
  (`constants.js:52,54,58`: INTEGER64/REAL64/UNSIGNED64), while `ESI_DT.bitsize`
  uses the *number* `64`. Today it's harmless because `dtype_bitsize` values are
  only string-interpolated in `objectlist.js` (output `"64"` is correct) — but
  any arithmetic on `dtype_bitsize[dt]` for a 64-bit type would string-concat.
  The port should use a numeric size table uniformly. (SUSPECTED-latent, not a
  live failure — INTEGER64 specs pass.)
- **`toEsiEepromH` emits `#endif __ESI_EEPROM_H__`** (`binaries.js:89`) — text
  after `#endif` without a comment is not valid ISO C (`-Werror` /
  `-pedantic` will complain). The golden test (`binariesSpecs.js`) pins this
  exact output, so the *test enshrines a mildly non-conforming artifact*. Low
  severity; note it so the port doesn't "fix" it and fail the reused vector.
- **`objectlist_getItemValue` VISIBLE_STRING** also returns the placeholder
  `'0'` rather than `objd.value` (same block as bug 1). Appears intentional
  (string data is carried by the separate `data` column), and the
  VISIBLE_STRING specs pass with value `0`, so not flagged as a bug — but the
  port should copy the behavior deliberately, not accidentally.
- **`getMaxMappings` counts a boolean as 2 mappings** but "array of booleans" is
  a `TODO` (`ecat_options.js:76`) — untested, likely wrong for arrays.
- **SM2/SM3 physical size written as 0** in the EEPROM SyncManager category
  (`EEPROM.js:252,261`). Common as a default but never validated against actual
  PDO length; unpinned.

---

## Appendix — reproduction

Harness (jsdom) that runs the whole suite headlessly and the probe scripts that
proved bugs 1–5 were written to the session scratchpad
(`harness.js`, `prove.js`, `prove2.js`). To re-run the suite:
`NODE_PATH=<repo>/node_modules node harness.js` after `npm install jsdom` in
`.cache/EEPROM_generator/`. Output: `TOTAL SPECS: 73 ... OVERALL: passed`.
