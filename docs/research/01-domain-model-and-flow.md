# Domain Model & Code Flow — JS EEPROM/SOES Generator

Reference: `/home/k1ase/App/soes_generator/.cache/EEPROM_generator/`. This report covers the domain model, project/config schema, code flow, constants, and validation, to inform a Rust port. All `file:line` citations are into that reference tree.

---

## 1. The Object Dictionary (OD) Data Model

### 1.1 Two representations exist

There are **two different in-memory shapes**, and the port must not conflate them:

1. **`odSections`** — the *editable* model, split into three sections, holding only user-authored objects. This is what the UI edits and what gets serialized to the project backup. Shape (`ui.js:58-62`, `od.js:17-23`):

```js
odSections = {
    sdo   : {},   // map<indexHexString, objd>
    txpdo : {},   // map<indexHexString, objd>
    rxpdo : {},   // map<indexHexString, objd>
}
```

2. **`od`** — the *flattened, complete* dictionary built at codegen time by merging mandatory objects + all three sections + synthesized PDO-mapping/SM-assignment objects. This is a single flat `map<indexHexString, objd>` and is what all generators consume (`od.js:282-291`).

**Index key format**: a hex string **without** `0x` prefix, **uppercase**, variable width, e.g. `'1000'`, `'1C12'`, `'6000'`. Produced by `indexToString` (`od.js:295-298`): `parseInt(index).toString(16).toUpperCase()`. Note: not zero-padded, so `0x100` → `'100'`. Address space scanned is `0x1000..0xFFFF` (`od.js:301-302`).

### 1.2 The `objd` (object description) shape

An `objd` is a plain object. Fields are **optional and otype-dependent**. Union of all fields seen across the code:

| Field | Type | Present on | Meaning |
|---|---|---|---|
| `otype` | string enum `OTYPE` | all | `'VAR'` \| `'ARRAY'` \| `'RECORD'` |
| `name` | string | all | Display + C-variable-name source |
| `dtype` | string enum `DTYPE` | VAR, ARRAY (not RECORD top-level) | CoE data type. RECORD has no top-level dtype; each subitem carries its own |
| `access` | string | VAR, ARRAY (and RECORD subitems) | `'RO'`\|`'RW'`\|`'WO'`\|`'RWpre'`. Default `'RO'` (`od.js:372`) |
| `value` | number\|string | VAR, subitems | Initial value. Numeric for numeric dtypes, string for `VISIBLE_STRING` |
| `data` | string | mandatory objects + injected at codegen | C-code pointer expression, e.g. `'&Obj.serial'`; auto-filled by `objectlist_link_utypes` (`objectlist.js:29-64`) |
| `size` | int | VAR only, `VISIBLE_STRING` only | Byte length of string; deleted for non-string types by `sizeCheckClear` (`od.js:61-65`). For ARRAY the modal reuses the `Size` field as element count |
| `items` | array | ARRAY, RECORD | Subitems; `items[0]` is always the `Max SubIndex` placeholder |
| `pdo_mappings` | string[] | PDO-mapped objects | List of PDO names (`'txpdo'`/`'rxpdo'`) this object is mapped into (`od.js:232-241`, `ui.js:395`) |
| `isSDOitem` | bool | set on SDO items at build | Marker set in `addSDOitems` (`od.js:97`) |

**VAR example** (from `getNewObjd`, `od.js:369-393` + mandatory `constants.js:131`):
```js
{ otype: 'VAR', dtype: 'UNSIGNED32', name: 'Device Type', access: 'RO', value: 0x1389 }
// VISIBLE_STRING VAR additionally carries: size: <int>
```

**ARRAY example** (`od.js:375-382`, `constants.js:142-148`):
```js
{ otype: 'ARRAY', dtype: 'UNSIGNED8', name: 'Sync Manager Communication Type', access: 'RO',
  items: [
    { name: 'Max SubIndex' },                      // items[0], subindex 0 — placeholder, no value
    { name: 'Communications Type SM0', value: 1 }, // subindex 1
    { name: 'Communications Type SM1', value: 2 },
    ...
  ]}
```
Array subitems (`getNewArraySubitem`, `od.js:400-407`) carry **only** `{ name, value }` — they inherit `dtype` from the parent `objd.dtype`. Array length is `items.length - 1`; managed by `setArrayLength` (`od.js:418-430`).

**RECORD example** (`constants.js:135-141`):
```js
{ otype: 'RECORD', name: 'Identity Object',   // NOTE: no top-level dtype
  items: [
    { name: 'Max SubIndex' },
    { name: 'Vendor ID',     dtype: 'UNSIGNED32', value: 600 },
    { name: 'Product Code',  dtype: 'UNSIGNED32' },
    { name: 'Revision Number', dtype: 'UNSIGNED32' },
    { name: 'Serial Number', dtype: 'UNSIGNED32', data: '&Obj.serial' },
  ]}
```
Record subitems (`getNewRecordSubitem`, `od.js:409-416`) carry `{ name, dtype, value }` and may also carry `access` (per-subitem, `ui.js:568,597`). Each record subitem has its **own** dtype.

### 1.3 Sub-index model

- `items[0]` is always the `Max SubIndex` entry — a bare `{ name: 'Max SubIndex' }` placeholder with no value/dtype. Real subitems start at index 1.
- Subindex numbering is the array position (1-based for real items). Displayed as 2-hex-digit `:0xNN` (`ui.js:629`).
- VAR objects have no `items`.

### 1.4 Access flags

Domain: `'RO'`, `'RW'`, `'WO'`, `'RWpre'` (from the HTML `Access` select, `index.html:83-88`). Default `'RO'`. Stored as string, no numeric encoding at this layer (encoding to CoE access bits happens inside generators, out of scope here).

### 1.5 PDO mapping flags

`objd.pdo_mappings` is an array of PDO section names (`'txpdo'` / `'rxpdo'` — the string constants, `constants.js:154-155`). Presence of a non-empty `pdo_mappings` marks the object as PDO-mapped. Set when the object is created in a PDO section (`od.js:394-396`) and appended during `addPdoMapping` (`od.js:232-241`). Only dtypes in `dtypes_PDO_allowed` may be PDO-mapped (validated `ui.js:445-448`).

---

## 2. Project / Config Schema (the `form`)

The config lives in the HTML form `SlaveForm` (`index.html:190`). `ui.js` reads it via named controls (`form.<Name>.value`). Defaults come from `getFormDefaultValues()` (`constants.js:183-218`) AND from the HTML `value=` attributes (`index.html`). Where they differ, HTML defaults win at page load then defaults are applied by `setFormValues` — see note below. Every backed-up control:

| Field name | Type | Meaning | Default (`constants.js`) |
|---|---|---|---|
| `VendorName` | string | ETG vendor name (ESI only) | `"ACME EtherCAT Devices"` |
| `VendorID` | hex/dec string | EtherCAT Vendor ID → OD `1018.1` | `"0x000"` |
| `ProductCode` | hex/dec string | Product code → OD `1018.2` | `"0x00ab123"` |
| `ProfileNo` | hex/dec string | CoE profile number (ESI) | `"5001"` |
| `RevisionNumber` | hex/dec string | Revision → OD `1018.3` | `"0x002"` |
| `SerialNumber` | hex/dec string | Serial → OD `1018.4` | `"0x001"` |
| `HWversion` | string | Hardware version → OD `1009` | `"0.0.1"` |
| `SWversion` | string | Software version → OD `100A` | `"0.0.1"` |
| `EEPROMsize` | hex/dec string (bytes) | EEPROM size in bytes | `"2048"` |
| `RxMailboxOffset` | hex string | RxMailbox (SM0) address | `"0x1000"` |
| `TxMailboxOffset` | hex string | TxMailbox (SM1) address | `"0x1200"` |
| `MailboxSize` | dec string (bytes) | Tx & Rx mailbox size; clamped to 128 for AX58100 (`ui.js:194-201`) | `"512"` |
| `SM2Offset` | hex string | SyncManager 2 address; RxPDO map base (`od.js:117-119`) | `"0x1400"` |
| `SM3Offset` | hex string | SyncManager 3 address; TxPDO map base (`od.js:136`) | `"0x1A00"` |
| `TextGroupType` | string | ESI group type (e.g. `DigIn`) | `"DigIn"` |
| `TextGroupName5` | string | ESI group description | `"Digital input"` |
| `ImageName` | string | EtherCAT configurator image name | `"IMGCBY"` |
| `TextDeviceType` | string | Product order code (ESI) | `"DigIn2000"` |
| `TextDeviceName` | string | Device name/description → OD `1008`; also project filename via `variableName` | `"2-channel Hypergalactic input superimpermanator"` |
| `Port0Physical` | enum string | Phys port 0 | `"Y"` |
| `Port1Physical` | enum string | Phys port 1 | `"Y"` |
| `Port2Physical` | enum string | Phys port 2 | `" "` |
| `Port3Physical` | enum string | Phys port 3 | `" "` |
| `ESC` | enum string | Slave chip; one of `SupportedESC` values | `SupportedESC.ET1100` (`"ET1100"`) |
| `SPImode` | enum string `"0".."3"` | SPI mode (CPOL/CPHA) | `"3"` |
| `CoeDetailsEnableSDO` | bool (checkbox) | CoE: enable SDO | `true` |
| `CoeDetailsEnableSDOInfo` | bool | CoE: enable SDO Info | `true` |
| `CoeDetailsEnablePDOAssign` | bool | CoE: enable PDO Assign | `false` |
| `CoeDetailsEnablePDOConfiguration` | bool | CoE: enable PDO Configuration | `false` |
| `CoeDetailsEnableUploadAtStartup` | bool | CoE: upload at startup | `true` |
| `CoeDetailsEnableSDOCompleteAccess` | bool | CoE: SDO complete access | `false` |
| `DetailsEnableUseFoE` | bool | Enable FoE | `false` |

**Port physical enum** (`index.html:256-286`): `"Y"`=MII, `"H"`=MII-Fast Hot Connect, `"K"`=EBUS, `" "` (space)=Not Used.

**Checkbox vs text distinction** (`backup.js:29-31`): a control is treated as boolean/checkbox iff its name starts with `"DetailsEnable"` or `"CoeDetailsEnable"`. For the Rust port these seven map to `bool`; all others are strings. Hex/dec numeric strings are parsed lazily with `parseInt` at consumption (accepts `0x` prefix), so the config struct can keep them as `String` and parse on demand, or parse eagerly to integers — but note the raw string is what gets round-tripped in backup.

### 2.1 Sync / Distributed Clocks config (`_dc`)

Separate from `form`. `_dc` is an **array** of sync-mode objects (`ui.js:63`). Each element (`addSyncClick`, `ui.js:711-723`):

```js
{
  Name:           "DcOff",        // string, must be unique case-insensitive (ui.js:687-692)
  Description:    "DC unused",    // string
  AssignActivate: "#x000",        // string (hex-ish token)
  Sync0cycleTime: 0,              // number
  Sync0shiftTime: 0,
  Sync1cycleTime: 0,
  Sync1shiftTime: 0,
}
```
Edited via `syncModal` (`ui.js:669-707`). Persisted in backup under key `dc`.

---

## 3. Code Flow: config + OD → generators

Entry point is `processForm(form)` (`ui.js:113-131`), triggered on every form `change` (`ui.js:88-91`) since `automaticCodegen = true` (`constants.js:14`).

### 3.1 Build phase — `buildObjectDictionary(form, odSections)` (`od.js:282-291`)

Produces the flat `od` in this exact order:

1. **`od = getMandatoryObjects()`** (`constants.js:129-151`) — seeds `1000, 1008, 1009, 100A, 1018, 1C00`.
2. **`populateMandatoryObjectValues(form, od)`** (`od.js:267-280`) — writes form values into mandatory objects:
   - `1008.value/size` ← `TextDeviceName`
   - `1009.value/size` ← `HWversion`
   - `100A.value/size` ← `SWversion`
   - `1018.items[1..4].value` ← `parseInt(VendorID/ProductCode/RevisionNumber/SerialNumber)`
3. **`addSDOitems(odSections, od)`** (`od.js:91-102`) — copies each `odSections.sdo` entry into `od`, sets `objd.isSDOitem = true`, calls `objectlist_link_utypes(objd)` (injects `.data` C-pointer strings), then `addObject`.
4. **`addTXPDOitems(...)`** (`od.js:131-139`) → `addPdoObjectsSection` with `pdo = { name:'txpdo', SMassignmentIndex:'1C13', smOffset: parseInt(SM3Offset) }`. Returns updated `booleanPaddingCount`.
5. **`addRXPDOitems(...)`** (`od.js:121-129`) → `addPdoObjectsSection` with `pdo = { name:'rxpdo', SMassignmentIndex:'1C12', smOffset: parseInt(SM2Offset) }`.

### 3.2 The PDO transform — `addPdoObjectsSection` (`od.js:161-265`)

For each user object in a PDO section, this **synthesizes additional OD entries**:

- Ensures a **SM PDO-assignment object** exists at `1C12` (rx) / `1C13` (tx): an `ARRAY` of `UNSIGNED16` named `Sync Manager <n> PDO Assignment` (`ensurePDOAssignmentExists`, `od.js:243-252`).
- For each mapped object, creates a **PDO-mapping RECORD** at a synthesized index starting from `smOffset` (`0x1400` rx / `0x1A00` tx) and incrementing per object. That record's items encode `0xINDEXSUBIDXBITSIZE` mapping values via `getPdoMappingValue` (`od.js:254-264`).
- Appends a `{ name:'PDO Mapping', value:'0x<offset>' }` assignment entry to the SM-assignment array.
- Marks the source object with `addPdoMapping` (adds to `pdo_mappings`).
- Handles per-dtype expansion: VAR → one mapping entry (+ padding entry if `BOOLEAN`); ARRAY → one entry per subitem; RECORD → one entry per subitem (+ boolean padding). Boolean padding entry uses `booleanPaddingBitsize = 7` (`constants.js:61`, `od.js:227-229`).
- Bit size per object from `varBitsize` (`od.js:147-153`): `ESI_DT[dtype].bitsize`, ×`size` for `VISIBLE_STRING`.

So the flat `od` handed to generators contains: mandatory objects + SDO items + PDO source objects + synthesized SM-assignment arrays (`1C12`/`1C13`) + synthesized per-object PDO-mapping records (at `0x14xx`/`0x1Axx`).

### 3.3 Generate phase (`ui.js:113-131`)

```js
const od = buildObjectDictionary(form, odSections);
const indexes = getUsedIndexes(od);          // sorted hex-string index list, 0x1000..0xFFFF (od.js:300-313)
outputCtl.objectlist.value  = objectlist_generator(form, od, indexes);
outputCtl.ecat_options.value = ecat_options_generator(form, od, indexes);
outputCtl.utypes.value      = utypes_generator(form, od, indexes);
outputCtl.HEX.hexData       = hex_generator(form);
outputCtl.HEX.value         = toIntelHex(hexData);
outputCtl.HEX.header        = toEsiEepromH(hexData);
outputCtl.ESI.value         = esi_generator(form, od, indexes, _dc);
outputCtl.backupJson        = prepareBackupFileContent(form, odSections, _dc);
saveLocalBackup(outputCtl.backupJson);
```

**In-memory representation passed to every generator**: `(form, od, indexes)` where `form` is the raw DOM form, `od` is the flat map, `indexes` is the ordered array of hex-string keys. ESI additionally gets `_dc`. Note `hex_generator` takes only `form` (not `od`). The Rust port's generator interface should mirror `(config, flat_od, ordered_indexes[, dc])`.

`objectlist_link_utypes` (`objectlist.js:29-64`) is the key transform that mutates objects, adding `.data` C-pointer strings:
- VAR → `&Obj.<varname>`
- ARRAY subitem → `&Obj.<varname>[<subindex-1>]`
- RECORD subitem → `&Obj.<varname>.<subitemvarname>`
`variableName()` (`validation.js:103-109`) derives the C identifier from the object name.

---

## 4. Constants / Enums (port these numeric values EXACTLY)

### 4.1 `OTYPE` (`constants.js:20-24`) — string-valued, not numeric here
`VAR='VAR'`, `ARRAY='ARRAY'`, `RECORD='RECORD'`. (CoE numeric object codes 0x07/0x08/0x09 are applied inside generators, not in this layer.)

### 4.2 `DTYPE` and derived tables

`DTYPE` values are the string names (`constants.js:26-45`). Implemented types (missing ones commented out): BOOLEAN, INTEGER8/16/32/64, REAL32/64, UNSIGNED8/16/32/64, VISIBLE_STRING.

**`ESI_DT`** (`constants.js:78-91`) — the authoritative table (name = ESI type, bitsize, C type):

| DTYPE | ESI name | bitsize | ctype |
|---|---|---|---|
| BOOLEAN | BOOL | 1 | uint8_t |
| INTEGER8 | SINT | 8 | int8_t |
| INTEGER16 | INT | 16 | int16_t |
| INTEGER32 | DINT | 32 | int32_t |
| INTEGER64 | LINT | 64 | int64_t |
| REAL32 | REAL | 32 | float |
| REAL64 | LREAL | 64 | double |
| UNSIGNED8 | USINT | 8 | uint8_t |
| UNSIGNED16 | UINT | 16 | uint16_t |
| UNSIGNED32 | UDINT | 32 | uint32_t |
| UNSIGNED64 | ULINT | 64 | uint64_t |
| VISIBLE_STRING | STRING | 8 | char |

**`dtype_bitsize`** (`constants.js:47-60`) duplicates bitsizes but note the **bug to preserve-or-fix decision**: 64-bit types are the *string* `'64'` not number `64` (INTEGER64/REAL64/UNSIGNED64). `ESI_DT` uses correct numeric 64. Prefer `ESI_DT.bitsize` (that's what `esiDTbitsize`/`varBitsize` use, `od.js:142-144`).

**`dtype_default_epmty_value`** (`constants.js:63-76`): 0 for most; `'0'` (string) for 64-bit types; `''` for VISIBLE_STRING. (Note misspelled identifier `epmty` — port under a corrected name.)

**`dtypes_PDO_allowed`** (`constants.js:93-116`) — Set of dtype names permitted in PDO mapping. Includes BIT1..BIT8, BITARR8/16/32 (not in main DTYPE enum), plus BOOLEAN, U/I 8/16/32/64, REAL32/64. Used at `ui.js:445`.

**`booleanPaddingBitsize = 7`** (`constants.js:61`).

### 4.3 `SupportedESC` (`constants.js:159-166`)
`AX58100`, `ET1100`, `LAN9252`, `LAN9253 Beckhoff`, `LAN9253 Direct`, `LAN9253 Indirect` (value strings shown). `configOnReservedBytes` set = {AX58100, all three LAN9253 variants} (`constants.js:169-174`). `ESCspecificSettings.AX58100.MaxMailboxSize = 128` (`constants.js:176-180`).

### 4.4 `SDO_category` (`constants.js:119-122`): `{ '1000':'m', '1009':'o' }` (mandatory/optional markers for CiA 301).

### 4.5 Section constants (`constants.js:153-156`): `sdo='sdo'`, `txpdo='txpdo'`, `rxpdo='rxpdo'`, `OD_sections=[sdo,txpdo,rxpdo]`.

### 4.6 SM assignment indices & PDO base offsets (hardwired in `od.js`)
- RxPDO → SM assignment `1C12`, map base `SM2Offset` (default `0x1400`).
- TxPDO → SM assignment `1C13`, map base `SM3Offset` (default `0x1A00`).
- New-object address range starts (`od.js:317-323`): sdo `0x2000`, txpdo `0x6000`, rxpdo `0x7000`.

### 4.7 Name sanitization char sets (`constants.js:221-222`)
`charsToReplace = [' ','.',',',';',':','/']` (→ `_`); `charsToRemove = ['+','-','*','=','!','@']` (→ removed).

---

## 5. Validation Rules Worth Preserving

From `validation.js` and `ui.js`:

**Value sanitization** (`sanitizeInitialValue`, `validation.js:58-74`), dispatched by dtype:
- `null` → `'0'` (always assign an initial value).
- VISIBLE_STRING → passthrough (all chars allowed).
- REAL32/REAL64 → `sanitizeFloat`: keeps `[0-9.]`, converts `,`→`.`, collapses to one decimal point, allows leading `-` and trailing `.`.
- INTEGER8/16/32/64 → `sanitizeInt`: keeps `[0-9]` + optional leading `-`.
- BOOLEAN → `sanitizeBool`: anything non-`'0'`/empty → `'1'`; else `'0'`.
- else (unsigned) → `sanitizeUint`: keeps `[0-9]` only, no sign.

**Hex sanitization**: `sanitizeHexa` (`validation.js:78-83`) uppercases, keeps `[0-9A-F]`. `sanitize0xHexa` (`validation.js:85-93`) prepends `0x`, strips a leading `0`. Index field is clamped to 6 chars (`ui.js:232`).

**Name rules**:
- `sanitizeString` (`validation.js:95-101`): trim + remove `charsToRemove` chars.
- `variableName` (`validation.js:103-109`): sanitizeString then replace `charsToReplace` with `_` → valid C identifier.
- Object-name uniqueness across all sections: `findObjectIndexByName` + check at save (`ui.js:434-438`) — reject if name used by a *different* index.
- Subitem-name uniqueness within an object: `checkIsSubitemNameFree` (`validation.js:111-123`, `ui.js:588`).

**Structural / type rules**:
- PDO-mapped object must use a dtype in `dtypes_PDO_allowed`, else reject (`ui.js:445-448`).
- Numeric VAR initial value must be `'0'` or numeric (`!isNaN`), else reject (`ui.js:456-459`).
- `VISIBLE_STRING` size must be ≥ initial-value length; warn/confirm otherwise (`ui.js:404-411`, `validation.js`; `getMinimumStringLength` = value length, `ui.js:249-251`).
- `size` field is stripped from non-string VARs (`sizeCheckClear`, `od.js:61-65`; `hasSize` true only for VISIBLE_STRING, `od.js:56-58`).
- Array length ≥ 1; `setArrayLength` maintains `items.length = length+1` (`od.js:418-430`).
- Remove-subitem guards: object must have `items`, enough items, and never drop below 1 real subitem (`ui.js:496-501`).
- Duplicate index on add → `alert` warning but still overwrites (`addObject`, `od.js:37-43`) — non-fatal.
- AX58100 clamps `MailboxSize` to ≤128 (`ui.js:194-201`).
- Sync-mode `Name` must be unique case-insensitive (`ui.js:687-692`).
- Backup validity: must have `.form` and `.od` or user confirms (`backup.js:16-23`).

---

## 6. Project Backup (serialization shape)

`prepareBackupObject` (`backup.js:33-51`) → JSON:
```js
{
  form: { <controlName>: value | checked, ... },  // strings, or bool for Details*/CoeDetails* controls
  od:   odSections,                                // { sdo:{}, txpdo:{}, rxpdo:{} } — editable model, NOT flat od
  dc:   [ <syncMode>, ... ],
}
```
Only non-button controls with truthy value are saved (`backup.js:25-27,38`). Restore: `loadBackup` (`backup.js:53-65`) restores the three sections + pushes `dc` entries + `setFormValues`. Persisted to `localStorage.etherCATeepromGeneratorBackup` on every codegen and downloadable as `esi.json`. **The flat `od` is never serialized — it is always rebuilt from `odSections` + `form`.**
