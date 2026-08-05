# 03 — Binary Image Generation, File I/O, and the XML Reader

Scope: `src/binaries.js`, `src/file_io.js`, `src/readers/xml_reader.js`, with
supporting context from `src/generators/EEPROM.js` (produces the byte array that
`binaries.js` transcodes) and `src/generators/esi_xml.js` (the writer that the
reader is meant to invert). Paths are relative to
`.cache/EEPROM_generator/`.

Key up-front finding: **`binaries.js` does not build the EEPROM layout** — it
only re-encodes an already-built `Uint8Array` into Intel HEX and a C header.
The actual byte/word layout lives in `EEPROM.js::hex_generator`. The Rust port
needs both, so both are documented here.

---

## 1. Binary image generation

### 1a. Byte/word layout (source: `EEPROM.js::hex_generator`)

The image is a `Uint8Array` of `form.EEPROMsize` bytes, pre-filled with `0xFF`
(`EEPROM.js:26-27`). All multi-byte integers are **little-endian**
(`writeEEPROMword_wordaddress` `EEPROM.js:334-338`, `writeEEPROMDword_wordaddress`
`EEPROM.js:340-346`). "Word address N" means byte offset `N*2`.

SII header, fixed offsets (`EEPROM.js:59-117`):

| Word addr | Bytes | Content | Line |
|-----------|-------|---------|------|
| 0 (b0) | PDI Control (0x05, or 0x80/0x82 for LAN9252/9253) | `EEPROM.js:60` |
| 0 (b1) | ESC config = 0x06 | `EEPROM.js:61` |
| 1 (b2) | SPI mode (`form.SPImode`, 0–3) | `EEPROM.js:62` |
| 1 (b3) | SYNC/LATCH config = 0x44 | `EEPROM.js:63` |
| 2 | Sync pulse length = 0x0064 | `EEPROM.js:64` |
| 3 | Extended PDI config = 0 | `EEPROM.js:65` |
| 4 | Configured Station Alias = 0 | `EEPROM.js:66` |
| 5 | Reserved / ESC-specific (`reserved_0x05`: 0, 0x001A for AX58100, 0xC040 for LAN9253) | `EEPROM.js:67`, set `33-57` |
| 6 | Reserved = 0 | `EEPROM.js:68` |
| 7 | **CRC8** over the first 14 bytes | `EEPROM.js:69-70` |
| 8–9 | VendorID (DWORD) | `EEPROM.js:85` |
| 10–11 | ProductCode (DWORD) | `EEPROM.js:86` |
| 12–13 | RevisionNumber (DWORD) | `EEPROM.js:87` |
| 14–15 | SerialNumber (DWORD) | `EEPROM.js:88` |
| 16–19 | Execution/Port0/Port1 delay + reserved = 0 | `EEPROM.js:90-93` |
| 20–23 | Bootstrap mailbox Rx/Tx offset+size (only if FoE enabled, else 0) | `EEPROM.js:95-105` |
| 24–27 | Standard mailbox Rx/Tx offset+size | `EEPROM.js:108-111` |
| 28 | Mailbox protocols: `0x04` (CoE) or `0x0C` (CoE+FoE) | `EEPROM.js:112`, `361-363` |
| 29–61 | Reserved, zero-filled | `EEPROM.js:113-115` |
| 62 | EEPROM size = `floor(EEPROMsize/128) - 1` | `EEPROM.js:83, 116` |
| 63 | Version = 1 | `EEPROM.js:117` |

Category (TLV) area starts at **byte offset 0x80** (`EEPROM.js:125`). Each
category is `[word: category type][word: length in words][data...]`:

- **Strings**, type `0x000A` (`EEPROM.js:153`). Layout `writeEEPROMstrings`
  `EEPROM.js:138-170`: 1 byte string-count, then per string `[len byte][chars]`.
  Strings written: DeviceType, GroupType, ImageName, DeviceName
  (`EEPROM.js:123`). Length field is `ceil(total/2)` words
  (`EEPROM.js:154`); a trailing pad byte is added when the byte count is odd
  (`EEPROM.js:149-152, 165-168`). Char codes via `charCodeAt` — **ASCII/Latin-1
  only, no UTF-8 handling** (`EEPROM.js:162`).
- **General**, type `0x1E` (30), fixed 0x10 words (`EEPROM.js:172-205`): string
  indices for group/image/order/name, CoE details byte
  (`getCOEdetails` `269-279`), FoE-enable byte, physical-port word
  (`getPhysicalPort` `281-302`), plus reserved/pad bytes.
- **FMMU**, type `0x28` (40) (`EEPROM.js:207-222`): 3 FMMU bytes (Outputs=1,
  Inputs=2, MBoxState=3) + 1 pad. Length hard-coded to 2 words.
- **SyncManager**, type `0x29` (41) (`EEPROM.js:224-268`): four 8-byte SM
  records (SM0 MbxOut, SM1 MbxIn, SM2 PDO out, SM3 PDO in), each
  `[start word][size word][control byte][0][enable byte][type byte]`.

Note the category area is written from `form` fields but the **object
dictionary itself is not stored in the binary** — only in the ESI XML and the
generated C. The Rust port's binary path only needs the `form`/SM/FMMU data.

### 1b. CRC / checksum

Two distinct checksums exist:

- **SII header CRC8** (`FindCRC` `EEPROM.js:305-322`): init `0xFF`, polynomial
  `0x07`, MSB-first, computed over the **first 14 bytes** (words 0–6), stored at
  word 7. This is the standard EtherCAT SII CRC-8/ETG.
- **Intel HEX record checksum** (`binaries.js:44-52`): two's complement of the
  low byte of the sum of every byte in the record line (byte count + address +
  type + data). `checksum = (0x100 - (sum % 0x100)) % 0x100` — note the code
  writes `0x100 - (sum%0x100)` then `.slice(-2)`, so a zero-sum edge yields
  `0x100`→`"00"` via the slice.

### 1c. Intel HEX format (`toIntelHex` `binaries.js:17-68`)

- **32 data bytes per record** (`bytes_per_rule = 32`, `binaries.js:19`).
- Record = `:` + byte-count (`"20"`) + 4-hex-digit **big-endian** address
  (`generate_hex_address` `56-67`) + record type `"00"` + data bytes + checksum +
  `\n` (`binaries.js:36-52`).
- Address = `rulenumber * 32` (`binaries.js:36`), truncated to 4 hex digits
  (`generate_hex_address` slices last 4 — **no extended-linear-address records**,
  so images > 64 KB would wrap the address silently; EEPROM sizes here are ≤ a
  few KB so it is fine in practice).
- EOF marker `":00000001FF"` appended (`binaries.js:30`), whole string
  upper-cased (`binaries.js:31`).
- **Assumes `record.length` is a multiple of 32** (`binaries.js:20`,
  `rulesTotalCount = length/32`). EEPROM sizes are powers of two ≥ 128, so this
  holds; a Rust port should still guard/pad.

### 1d. C header (`toEsiEepromH` `binaries.js:71-96`)

`unsigned char esiEepromData[] = { ... };` — **16 bytes per line**, each byte
`0xNN` uppercase 2-digit (`toByte` `92-95`), comma-separated. Trailing line
`\n};\n#endif __ESI_EEPROM_H__` (`binaries.js:89`) — the `#endif` has a bare
token after it (harmless, not a valid comment; reproduce verbatim if byte-exact
output matters).

### 1e. ConfigData string (`getConfigDataString` `EEPROM.js:348-357`)

Separate from the full image: emits the first **14 bytes** (if ESC ∈
`configOnReservedBytes`: AX58100, LAN9253 variants — `constants.js:169-174`)
else **7 bytes**, as uppercase hex, no separators. Reached via
`hex_generator(form, true)` (`EEPROM.js:16-20`) and embedded in the ESI
`<ConfigData>` element (`esi_xml.js:216-217`).

---

## 2. File I/O (`file_io.js`)

Browser-only (`Blob` + anchor-click download, `downloadFile` `file_io.js:17-24`;
`FileReader` restore `readFile` 27-35). A Rust port replaces these with plain
filesystem writes.

**Two output paths, and they differ:**

`downloadGeneratedFilesZipped` (`file_io.js:37-53`) — JSZip bundle `esi.zip`
containing **8 files**:

| Zip entry | Source | Format |
|-----------|--------|--------|
| `{projectName}.xml` | `result.ESI.value` | ESI XML |
| `eeprom.hex` | `result.HEX.value` | Intel HEX string |
| `eeprom.bin` | `result.HEX.hexData` | **raw `Uint8Array`** (the actual binary image) |
| `eeprom.h` | `result.HEX.header` | C header |
| `ecat_options.h` | `result.ecat_options.value` | SOES C |
| `objectlist.c` | `result.objectlist.value` | SOES C |
| `utypes.h` | `result.utypes.value` | SOES C |
| `esi.json` | `result.backupJson` | JSON project backup |

`downloadGeneratedFiles` (`file_io.js:55-63`) — individual downloads.
**Asymmetry: this path omits `eeprom.bin`** (no raw binary), and content-types
differ (XML as `text/html` `line 56`, hex as `application/octet-stream`
`line 57`). The backup goes out as `esi.json` via `downloadBackupFile`
(`backup.js:109-111`).

Wiring (`ui.js:122-124`): `HEX.hexData = hex_generator(form)` (the `Uint8Array`),
`HEX.value = toIntelHex(hexData)`, `HEX.header = toEsiEepromH(hexData)`. So the
`.bin` is the raw array and `.hex`/`.h` are transcodings of it.

For the Rust port: emit `.bin` (raw bytes), `.hex` (Intel HEX), `.h` (C header),
`.xml` (ESI), the three SOES C files, and `.json` backup. `projectName` is the
only variable filename; the rest are fixed literals.

---

## 3. XML reader (`readers/xml_reader.js`) — reverse mapping

**Uses `DOMParser` (real DOM), not regex** (`xml_reader.js:19-20`) — good, order
tolerant for the fields it reads. But it is **incomplete and effectively a
stub**: it is referenced only in `tests.html:22`, never wired into the app
(`grep` shows no call site outside tests). Real project persistence is the JSON
backup (`backup.js`), not this reader.

### What is parsed → `result.form.*`

| XML source | Field | Line |
|-----------|-------|------|
| `Vendor/Name` | `VendorName` | `25` |
| `Vendor/Id` (decimal → hex) | `VendorID` | `26` |
| `Group/Type` | `TextGroupType` | `30` |
| `Group/Name` | `TextGroupName5` | `31` |
| `Device/Type` text | `TextDeviceType` | `37` |
| `Device/Type@ProductCode` (`#x..`→`0x..`) | `ProductCode` | `38` |
| `Device/Type@RevisionNo` | `RevisionNumber` | `39` |
| `Device/Name` | `TextDeviceName` | `44` |
| `Profile/ProfileNo` | `ProfileNo` | `73` |
| `Mailbox/CoE@*` (SdoInfo, PdoAssign, PdoConfig, PdoUpload, CompleteAccess) | `CoeDetails*` | `84-91` |
| `Eeprom/ByteSize` | `EEPROMsize` | `65` |
| — | `ESC` (passed as arg) | `18` |
| — | `SPImode` hard-coded `'0'` | `68` |

Hex conversion helpers: `toJsHex` (`97-105`) strips leading `#`, prepends `0`
for `x..`; `toHex` (`107-109`) is `parseInt(str).toString(16)` — decimal-or-hex
in, `0x`-prefixed lowercase out.

### What is IGNORED / lossy (critical for round-trip)

- **The entire Object Dictionary.** `addDeviceProfile` (`71-79`) fetches the
  `DataTypes` and `Objects` elements into locals and does nothing — `TODO read
  OD` (`line 78`). `result.od` stays `{sdo:{}, rxpdo:{}, txpdo:{}}` (`line 17`).
- **PDOs / SM / FMMU / DC.** `DeviceFmmu`, `DeviceSm`, `DeviceRxPdo`,
  `DeviceTxPdo` (`48-51`) and `DeviceDc` (`56`) are queried but their results are
  **never used**. `result.dc` stays `[]`.
- **ConfigData not decoded** — `TODO read SPI mode etc` (`67`); `SPImode` forced
  to `'0'`.
- **Never read:** SerialNumber, HW/SW version (`1009`/`100A`), mailbox
  offsets/sizes, SM2/SM3 offsets, ImageName, Port physical (`Physics` attr),
  FoE enable, EoE.

### Fragility / bugs

- **`getElementsByTagGroupType` (`line 41`) is not a DOM method** — this
  fallback (only reached when `TextDeviceType` is empty) would throw. Dead-ish
  but a latent crash.
- **No null-guards** except the `CoE` block (`84`). Missing `Vendor`, `Group`,
  `Profile`, or `Eeprom` elements throw immediately. Assumes `[0]` singletons and
  element presence.
- Uses `.innerHTML` on XML nodes (`25, 30, 37, …`) rather than `textContent` —
  works in browsers but is quirky and would need `text()`/`textContent` in a
  port.
- **Attribute-name mismatch:** reader writes
  `CoeDetailsEnablePdoUploadAtStartup` (`line 90`) but the writer/EEPROM use
  `CoeDetailsEnableUploadAtStartup` (`EEPROM.js:276`) — the round-tripped field
  name does not match the field the rest of the app consumes, so this bit is
  silently lost.
- `RevisionNumber` yields `"0x1"` where the JSON backup used `"0x001"`
  (`spec` line 7 vs assert line 1330) — hex normalization is lossy on
  leading zeros.

---

## 4. Round-trip fidelity (writer `esi_xml.js` vs reader `xml_reader.js`)

**Write-then-read does NOT reproduce the project.** The writer serializes the
full OD (DataTypes, Objects, PDOs, SM, FMMU, DC, ConfigData); the reader
recovers only a handful of identity/header `form` fields and drops everything
else. Concretely, after `esi_xml → xml_reader`:

- Recovered: VendorName/ID, Group type/name, Device type/name, ProductCode,
  RevisionNumber, ProfileNo, EEPROMsize, CoE detail flags, ESC (external).
- Lost: **all OD objects and PDO mappings**, DC config, SM/FMMU offsets, mailbox
  offsets/sizes, SerialNumber, HW/SW versions, ImageName, port physicals, SPI
  mode, FoE/EoE.

The reader's own test only asserts the ~9 header fields and **comments out
everything else** (`spec/readers/xml_reader_cia402exampleSpecs.js:1331-1357`),
confirming it is a work-in-progress, not a faithful inverse.

**Implication for the Rust port:** treat the ESI XML as a *write-mostly* export
format. Authoritative round-trip persistence is the `esi.json` backup
(`backup.js` / `prepareBackupFileContent`), which stores the whole
`{form, od, dc}` model losslessly. If the Rust port needs true ESI-XML
round-trip (re-importing a hand-edited or third-party ESI), the OD/PDO/SM/DC
parsing in `xml_reader.js` must be **written from scratch** — it does not exist
today. The one structural asymmetry to preserve if you do implement it: the
writer emits `#x`-prefixed hex and decimal Vendor Id; the reader must normalize
both (`toJsHex`/`toHex`) and should use `textContent`, add null-guards, and fix
the `getElementsByTagGroupType` and `PdoUploadAtStartup` naming bugs.
