# Generators — research for the Rust port

Reference: `.cache/EEPROM_generator/src/generators/*.js` plus shared helpers in
`src/constants.js`, `src/od.js`, `src/validation.js`.

All five generators are pure string/byte builders. They take:
- `form` — a flat bag of `{value|checked}` fields (the HTML form). Values are
  strings; numbers come in as `"0x1400"` etc. and are `parseInt`-ed at use.
- `od` — the fully-built Object Dictionary, keyed by uppercase hex index string
  (`"1018"`, `"1C12"`), each entry an `objd` (see below).
- `indexes` — array of the OD's used index strings, ascending (`0x1000..0xFFFF`).
- `dc` — Distributed Clock op-mode list (ESI only).

The OD is built once by `buildObjectDictionary` (`od.js:282`) before any
generator runs. Each `objd` has: `otype` (VAR/ARRAY/RECORD), `dtype`
(DTYPE.*), `name`, optional `value`, `size` (string length), `access`
(`RO`/`RW`/`WO`/`RWpre`), `pdo_mappings` (array of `"rxpdo"`/`"txpdo"`),
`items` (subitems for ARRAY/RECORD; item[0] is always "Max SubIndex"),
`isSDOitem`, and `data` (a C expression like `&Obj.foo`, injected by
`objectlist_link_utypes`, `objectlist.js:29`). Porting the generators means
porting this `objd` model first.

Shared lookup tables (`constants.js`):
- `dtype_bitsize` (`:47`) — bit size per DTYPE for objectlist.c. **Footgun:**
  the 64-bit and REAL64 entries are the *strings* `'64'`, not numbers 64
  (`:52,54,58`). Works in JS template strings; a Rust port must decide the
  numeric type deliberately.
- `ESI_DT` (`:78`) — per DTYPE `{ name (IEC 61131 type), bitsize, ctype }`.
  Used by ESI, utypes, and objectlist. This is the single source of truth for
  type mapping; port it as one table.
- `SDO_category` (`:119`) — `{'1000':'m','1009':'o'}`, CiA-301 category letters
  for ESI `<Category>`.
- `booleanPaddingBitsize = 7` (`:61`) — BOOLEANs in a PDO are padded to a full
  byte with a 7-bit filler entry.
- `SupportedESC` / `configOnReservedBytes` (`:159,169`) — ESC chip variants and
  which ones write config bytes past byte 7.

---

## 1. EEPROM.js — SII / EEPROM binary (HIGHEST PRIORITY)

### Output artifact
Produces the SII (Slave Information Interface) EEPROM image as a
`Uint8Array` of `EEPROMsize` bytes (default 2048), pre-filled with `0xFF`.
Downloaded as `.bin`/`.hex` and flashed to the ESC's EEPROM chip; the
EtherCAT master and the ESC read it at startup. `hex_generator(form, true)`
returns just the ConfigData hex string (first 7 or 14 bytes) which the ESI
XML embeds in `<Eeprom><ConfigData>`.

### Input
Almost entirely `form` fields: VendorID/ProductCode/RevisionNumber/
SerialNumber, mailbox offsets/size, SM2/SM3 offsets, ESC type, SPI mode,
EEPROMsize, the four text strings, CoE detail checkboxes, port physicals,
FoE flag. It does **not** walk the OD at all — PDO/SM detail comes from the
fixed form config, not from `od`/`indexes`.

### Byte layout — exact word map
Everything little-endian. Word address N == byte offset 2N. Helpers
(`EEPROM.js:324-346`):
- `writeEEPROMbyte_byteaddress(b, addr)` → `record[addr]=b`
- `writeEEPROMword_wordaddress(w, addr)` → bytes `[2*addr]=w&0xFF`, `[2*addr+1]=w>>8`
- `writeEEPROMDword_wordaddress(dw, addr)` → 4 LE bytes at `2*addr`

**Config area, words 0–7** (`:59-70`), maps to ESC registers:
| Word | Bytes | Content |
|------|-------|---------|
| 0 | 0 / 1 | PDI control (0x05, or 0x80/0x82 for LAN9252/9253) / ESC config = 0x06 |
| 1 | 2 / 3 | SPI mode (0–3) / SYNC-LATCH config = 0x44 |
| 2 | — | SyncSignal pulse length = 0x0064 |
| 3 | — | Extended PDI config = 0 |
| 4 | — | Configured Station Alias = 0 |
| 5 | — | Reserved, ESC-specific (`reserved_0x05`: 0x001A AX58100, 0xC040 LAN9253) |
| 6 | — | Reserved = 0 |
| 7 | — | **CRC** of bytes 0..13, see below |

Per-ESC branching sets `pdiControl` and `reserved_0x05` (`:33-57`). Magic
values documented inline; port as a match on `SupportedESC`.

**CRC** (`FindCRC`, `:305`): 8-bit CRC over the **first 14 bytes**
(`FindCRC(record,14)`), poly `0x07`, init `0xFF`, MSB-first, masked to 8 bits
each round. This is CRC-8/attribute per ETG. Result stored in word 7.
`getConfigDataString` (`:349`) re-emits either 7 or 14 bytes as uppercase hex
depending on `configOnReservedBytes.has(esc)` — for AX58100/LAN9253 the ESI
ConfigData is 14 bytes, otherwise 7.

**Identity, words 8–15** (`:85-88`): VendorID(8), ProductCode(10),
RevisionNumber(12), SerialNumber(14) — each a DWORD (2 words).

**Words 16–23** (`:90-104`): execution/port delays = 0; bootstrap mailbox
(RxOff/Size/TxOff/Size) only if FoE enabled, else zeros.

**Words 24–63** (`:108-117`): standard mailbox RxOff(24) RxSize(25) TxOff(26)
TxSize(27); protocols word(28) = 0x0C if FoE else 0x04; words 29–61 zeroed;
word 62 = EEPROM size encoded as `floor(bytes/128)-1` (`:83`); word 63 = 1.

### Category section (word 0x80 = byte 256 onward)
Written by four functions that thread a running byte `offset`. Each TLV
category = word[type] + word[length-in-words] + payload. `offset` is byte-based
but `offset/2` is passed as a word address everywhere — **offset must stay even
or the division corrupts alignment.**

1. **STRING category** (type `0x0A`, `writeEEPROMstrings`, `:138`): the 4
   strings `[DeviceType, GroupType, ImageName, DeviceName]`. Layout: length
   word = `ceil(total/2)` where total = sum(string lengths) + 1 byte per string
   (its length prefix) + 1 byte (string count). Then count byte, then each
   string as `[len][chars...]`. Pads one `0x00` if the running total is odd
   (`length_is_even` logic at `:149`, note the flag name is inverted vs. its
   value). String indices are **1-based** and referenced by the General
   category. `charCodeAt` → raw byte (ASCII assumed).

2. **GENERAL category** (type `0x1E`=30, `writeEEPROMgeneral_settings`,
   `:172`): fixed `categorysize = 0x10` words. Zeroes the region first, then
   writes string indices GroupInfo=2, ImageName=3, OrderNumber=1, DeviceName=4
   (`:185-188`), a reserved byte, CoE details byte (`getCOEdetails`, bit flags
   `:269`), FoE-enable byte, EoE=0, 3 reserved, flags byte, current-consumption
   word, 2 pad, physical-port word (`getPhysicalPort`, `:281` — 4 nibbles, one
   per port: MII=0x01, EBUS=0x03, none=0), then **14 pad bytes** (`:203`) that
   are advanced by `offset += 14` but not written (region was pre-zeroed).

3. **FMMU category** (type `0x28`=40, `writeFMMU`, `:207`): length=2 words,
   payload bytes `[1,2,3,0]` = Outputs, Inputs, MailboxState, padding.
   Hard-coded to 3 FMMUs.

4. **SYNCMANAGER category** (type `0x29`=41, `writeSyncManagers`, `:224`):
   length `0x10` words, four 8-byte SM records SM0–SM3. Each: start-addr word,
   size word, control byte, status(0), enable(1), SM-type byte. SM0=MbxOut
   (ctrl 0x26,type1), SM1=MbxIn (0x22,type2), SM2=Outputs (0x24,type3, size 0),
   SM3=Inputs (0x20,type4, size 0). Addresses from form offsets.

**Note:** no PDO categories (0x32/0x33 TxPDO/RxPDO) are emitted despite the
task mentioning them — this generator omits them; PDO description lives only in
the ESI XML and objectlist. There is also **no category-end terminator (0xFFFF)
written explicitly** — the `0xFF` pre-fill provides it.

### Fragile bits to port
- CRC-8 loop (poly 0x07, init 0xFF, over 14 bytes) — get the masking exact.
- `offset`/`offset/2` dual addressing; any odd offset silently misaligns.
- STRING length/padding parity math (`:147-168`).
- The 14 "phantom" pad bytes in General relying on pre-zeroed memory.
- Per-ESC magic register values; EEPROMsize `floor(n/128)-1` encoding.
- 1-based string index cross-references between STRING and GENERAL categories.

---

## 2. esi_xml.js — ESI / ESD device description XML

### Output artifact
A single XML string (`EtherCATInfo` per ETG.2000), downloaded as the `.xml`
ESI file. Consumed by EtherCAT masters (TwinCAT etc.) to configure the slave.
Embeds the EEPROM ConfigData via `hex_generator(form,true)` (`:216`).

### Input
`form` (vendor/device/group text, ports, mailbox/SM offsets, CoE checkboxes,
FoE, ProfileNo), the full `od`+`indexes` (DataTypes, Objects, PDOs), and `dc`
(op modes).

### Algorithm
One big template-literal concatenation, four phases:
1. **DataTypes** (`:49-131`): `addObjectDictionaryDataType` per index. VARs get
   queued into `variableTypes` (dedup by ESI type name incl. `STRING(n)`);
   ARRAY/RECORD emit a `<DataType>` with `DT{index}` name, subitems, bit
   offsets, and PDO-mapping flags. ARRAYs also emit a `DT{index}ARR` helper
   type. RECORD subitem bit offsets accumulate from 16 (`:86-97`).
2. **Objects** (`:133-172`): `addDictionaryObject` — index, name, type, bitsize,
   default value/string, subitems, Access/PdoMapping/Category flags.
3. **SM + PDOs** (`:176-205`): fixed FMMU/SM lines; then walks `od` for objects
   mapped to rxpdo/txpdo and emits `<RxPdo>/<TxPdo>` with a per-object memory
   offset counter (starts at SM2/SM3 offset, `++` per PDO object).
4. **Mailbox/DC/Eeprom** tail (`:208-219`).

Sizes via `esiBitsize` (`:377`) / `varBitsize` (`od.js:147`): VAR = dtype bits
(× size for strings); ARRAY = 2 max-subindex bytes + elements×dtype; RECORD =
16 + Σ subitem bits (+7 per BOOLEAN).

### Fragile bits
- BitOffs accumulation in RECORD/ARRAY DataTypes must match objectlist/utypes
  exactly or the master mis-parses PDOs.
- BOOLEAN padding emits a stray 7-bit `<Entry>` with Index/SubIndex 0 (`:268`).
- `Physics=` string concatenation has a bug: `|| + form.Port3Physical.value`
  (`:26`) — a JS quirk to replicate-or-fix consciously.
- Multiple PDO mappings per object `alert()` and silently use only the first
  (`:289`).
- Manual XML string building = no escaping of `&`/`<`/`>` in user text. A Rust
  port should escape (the JS doesn't).

---

## 3. objectlist.js — SOES objectlist.c

### Output artifact
`objectlist.c` C source: the SOES object dictionary (`_objd[]` per object +
`SDOobjects[]` master table). Compiled into the SOES slave stack.

### Input
`od` + `indexes`. Relies on `objd.data` (the `&Obj.x` link set earlier by
`objectlist_link_utypes`, `:29`).

### Algorithm (`:66`)
Emits header includes, then per index: a `acName{index}[]` string (and
`acName{index}_{subidx}` for subitems, subindex zero-padded to 2 digits by
`subindex_padded`, `:198`), then a `_objd SDO{index}[]` array, then the
`SDOobjects[]` dictionary table terminated with
`{0xffff,0xff,0xff,0xff,NULL,NULL}` (`:89`).

Per-otype `_objd` rows (`:112`):
- VAR: one row `{0x0, DTYPE_x, bitsize, flags, name, value, data}`.
- ARRAY: max-subindex row `{0x00, DTYPE_UNSIGNED8, 8, ATYPE_RO, name_00, count, NULL}`
  then one row per element using `objd.dtype`.
- RECORD: max-subindex row then one row per subitem using each subitem's dtype.

`bitsize` from `get_objdBitsize` (`:16`) = `dtype_bitsize[dtype]`, ×`size`
for strings. Flags via `objectlist_objdFlags` (`:211`): `ATYPE_{access}`
(default RO) OR-ed with `ATYPE_{RXPDO|TXPDO}` per mapping. Data via
`objectlist_objdData` (`:225`): the `&Obj.` link, or a quoted string literal
for VISIBLE_STRING.

### Fragile bits
- `dtype_bitsize` string-vs-number `'64'` issue leaks into output.
- `objectlist_getItemValue` **bug** (`:183`): passes local `value` (always
  `'0'`) to `float32ToHex`/string return instead of `objd.value` — the default
  branch works, REAL32/STRING paths are broken. Decide whether to port or fix.
- `DTYPE_{name}` / `ATYPE_{name}` / `OTYPE_{name}` are literal C macro names
  built by string interpolation from the enum values (`constants.js` DTYPE
  strings are the macro suffixes) — the enum string values ARE the ABI.
- Subindex zero-padding to 2 hex digits; index used raw as hex.

---

## 4. ecat_options.js — ecat_options.h

### Output artifact
`ecat_options.h` C header of `#define`s for the SOES stack (mailbox/SM
addresses, mapping counts, PDO buffer sizes).

### Input
`form` (FoE, MailboxSize, Rx/Tx offsets, SM2/SM3 offsets) + `od`/`indexes`
(only to count PDO mappings).

### Algorithm (`:16`)
Straight-line `#define` emission. USE_FOE from checkbox; MBXSIZE = MailboxSize;
MBX0/MBX1 (+ `_b` bootstrap variants) start/len/end/control — control bytes
0x26 (Rx) / 0x22 (Tx); SM2 ctrl 0x24, SM3 ctrl 0x20; offsets via
`indexToString` (hex, no `0x`, uppercased, `od.js:295`). `MAX_MAPPINGS_SM2/3`
from `getMaxMappings` (`:64`): counts PDO-mapped subitems (or the VAR itself),
**+1 extra per BOOLEAN** (padding counts as a mapping). `MAX_RXPDO_SIZE` /
`MAX_TXPDO_SIZE` hard-coded 512 (TODO in source to compute from actual size).

### Fragile bits
- BOOLEAN double-counting in mapping count must match ESI/od padding logic.
- Fixed 512 PDO buffer — a correctness risk if real PDO > 512 bytes; the port
  could compute it, but matching the original means hard-coding.
- Whitespace alignment is cosmetic only.

---

## 5. utypes.js — utypes.h

### Output artifact
`utypes.h`: the `_Objects` C struct (typedef + `extern _Objects Obj;`) holding
the process-data and parameter storage the objectlist `&Obj.x` pointers refer
to. Compiled into SOES.

### Input
`od` + `indexes`. Sections by role: always `serial` (uint32); Inputs =
txpdo-mapped objects; Outputs = rxpdo-mapped; Parameters = `objd.isSDOitem`.

### Algorithm (`:16`)
`getUtypesDeclaration` (`:60`) per object, C type from `ESI_DT[dtype].ctype`:
- VAR → `ctype name;` (`char name[size];` for VISIBLE_STRING).
- ARRAY → `ctype name[items.length-1];`.
- RECORD → anonymous `struct { ... } name;` with a field per subitem.
Names sanitized by `variableName` (`validation.js:103`): replaces
` .,;:/` → `_`, strips `+-*=!@`. Inputs/Outputs sections only emitted if
non-empty (`isPdoWithVariables`).

### Fragile bits
- Struct field order must exactly match objectlist `&Obj.field` references and
  ESI/PDO bit offsets — three generators share this implicit contract.
- `variableName` sanitization rules must be identical across utypes and
  objectlist or the `&Obj.x` links won't resolve.
- Multiple-PDO-mapping objects only `alert`, not handled.
- No struct packing/alignment attributes emitted — relies on `cc.h`.
