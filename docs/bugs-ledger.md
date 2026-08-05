# Bug ledger — JS reference vs. Rust port decisions

Bugs found in the JavaScript reference (`.cache/EEPROM_generator`, commit `6f26c1b`).
Each **CONFIRMED** bug was reproduced headlessly (input → expected → actual) — see
[`research/repro/`](research/repro/). The **Port decision** column is what the Rust
port should do; "fix" means diverge from the reference on purpose (and add the missing
test the reference lacks).

## Confirmed (reproduced)

| # | Sev | Location | Bug | Proven actual | Port decision |
|---|-----|----------|-----|---------------|---------------|
| 1 | med-high | `objectlist.js:183` `objectlist_getItemValue` | REAL32 default value encoded from local placeholder `'0'`, not `objd.value` → every REAL32 default emitted as `0x00000000` | `value=1.5` → `...DTYPE_REAL32, 32, ..., 0x00000000` | **Fix.** Encode IEEE-754 of `objd.value` (`1.5`→`0x3FC00000`). No REAL32 spec exists — add one. |
| 2 | med | `xml_reader.js:90` | CoE "upload at startup" written to key `CoeDetailsEnablePdoUploadAtStartup`; rest of app uses `CoeDetailsEnableUploadAtStartup` → 0x10 CoE bit lost on ESI→form restore | correct key `undefined`, wrong key `true` | **Fix** (if the reader is ported at all — see note). Use the canonical key. |
| 3 | med | `xml_reader.js:41` | `getElementsByTagGroupType` is not a DOM method (typo for `getElementsByTagName`); throws when `<Type>` is empty | `TypeError: ...is not a function` | **Fix** (if reader ported). |
| 4 | med | `esi_xml.js:26` | `Physics="P0+P1+P2 \|\| +P3"` — precedence makes `\|\|` branch dead, Port3 never included; disagrees with EEPROM `getPhysicalPort` which uses all 4 ports | ports `Y Y Y H` → `Physics="YYY"` | **Fix.** Emit all 4 ports; make ESI and EEPROM agree. Note: the default/CiA-402/FoE/VAR ESI vectors pin `Physics="YY "` (3 chars) — the fix makes them `"YY  "`, so those ESI golden strings are regenerated, not reused verbatim. |
| 5 | low-med | `binaries.js:17` `toIntelHex` | Assumes `record.length % 32 == 0`; partial final record reads past array end | `EEPROMsize="2000"` → `TypeError: ...reading 'toString'` | **Fix.** Validate/pad `EEPROMsize` to a multiple of 32 (ideally 128 — `EEPROM.js` word 62 = `floor(n/128)-1`). Reject or round with a clear error. |
| 6 | low-med | `binaries.js:52` `toIntelHex` | Checksum byte not zero-padded: `.slice(-2)` of a `<0x10` two's-complement value leaves a single hex digit → malformed Intel-HEX record | checksum `0x07` → record ends `...FF7` (74 chars) not `...FF07` (75) | **FIXED** (post-merge): the port emits a 2-digit checksum `(0x100 - sum) & 0xff` as `{:02x}`; the three `eeprom.hex` goldens were regenerated (`dump_golden.js` `patchDivergences`). Verified: SOEM `eepromtool` (`input_intelhex`) parses the fixed goldens rc=1. See [`faithful-port-quirks.md`](faithful-port-quirks.md) #1. |

## Latent / decide-deliberately (not live failures)

| Location | Issue | Port decision |
|----------|-------|---------------|
| `constants.js:52,54,58` `dtype_bitsize` | 64-bit / REAL64 sizes stored as **string** `'64'` not number `64`; harmless only because output is string-interpolated | Use **one numeric** bitsize table (`ESI_DT.bitsize`) everywhere; never string-concat sizes. |
| `binaries.js:89` `toEsiEepromH` | Emits `#endif __ESI_EEPROM_H__` — bare token after `#endif` (not valid ISO C under `-pedantic`) | **FIXED** (post-merge): the port emits the conformant `#endif /* __ESI_EEPROM_H__ */`; the `eeprom.h` goldens were regenerated to match (`dump_golden.js` `patchDivergences`). See [`faithful-port-quirks.md`](faithful-port-quirks.md) #2. |
| `objectlist.js:183` VISIBLE_STRING branch | Returns placeholder `'0'` not `objd.value` — **intentional** (string data travels in the separate `data` column); specs pass | Copy behavior **deliberately**, with a comment. |
| `ecat_options.js:76` `getMaxMappings` | Counts a BOOLEAN as 2 mappings (padding); "array of booleans" is a TODO, likely wrong for arrays | Port the byte-exact single-boolean behavior; treat boolean-array as unsupported (match reference limitation) until specified. |
| `esi_xml.js` / `utypes.js` | Multiple PDO mappings per object → `alert()` and silently use only the first | Reference limitation ("single non-dynamic PDO per direction"). Make it an explicit typed error in the port, not a silent drop. |
| Manual XML build (`esi_xml.js`) | No escaping of `& < >` in user-supplied text | **Fix.** Escape XML text/attributes. (Will differ from reference only when input contains those chars — none of the golden vectors do, so vectors still pass.) |
| `EEPROM.js:252,261` | SM2/SM3 physical size written as `0` in SyncManager category; never validated against actual PDO length | Match reference default; note as a known gap. |

## Note on the XML reader

`xml_reader.js` is an **incomplete stub** (referenced only by tests; drops the entire OD,
PDOs, SM, FMMU, DC — `TODO read OD`). Bugs 2 & 3 live in code the app never calls. The
authoritative round-trip format is the **JSON backup** (`backup.js`), which is lossless.
**Recommendation:** the Rust port should use JSON for round-trip and *not* port
`xml_reader.js` as-is; only implement true ESI-XML import if re-importing third-party ESI
files is a hard requirement, and then write the OD parser from scratch (see
[`research/03-binaries-io-readers.md`](research/03-binaries-io-readers.md) §4).
