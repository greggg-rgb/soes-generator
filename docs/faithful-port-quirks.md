# Faithful-port quirks — inherited behaviors that look like bugs

During the port, several behaviors were flagged as *possible* bugs, then found to be
**faithful copies of the JS reference** (`.cache/EEPROM_generator`, commit `6f26c1b`)
rather than porting mistakes. Because the port is validated **byte-for-byte** against
golden vectors dumped from that reference, copying these verbatim is what makes the
goldens pass — but "matches the reference" and "is correct" are not the same thing.

This doc maps each one so we can decide, deliberately, whether it's a **real bug we
inherited**, a **harmless quirk**, or **correct-by-design**. It complements
[`bugs-ledger.md`](bugs-ledger.md), which covers the 5 bugs we *did* fix and the
already-cataloged latent issues.

## Verdict legend

- 🔴 **Inherited bug** — the reference is wrong; we copied it to match the golden. Real-world impact; candidate for a deliberate fix (which would change that golden).
- 🟡 **Quirk / limitation** — technically imperfect or conservative, but low or no real impact. Leave unless a concrete need appears.
- 🟢 **Correct by design** — looks odd, but is intentional and right.

## Summary

| # | Behavior | Rust site | JS ref | Verdict | Real-world impact |
|---|----------|-----------|--------|---------|-------------------|
| 1 | Intel-HEX checksum not zero-padded (1 hex digit when < 0x10) | `generators/intel_hex.rs:42-49` | `binaries.js:52` | 🔴 **Inherited bug** | Malformed `.hex` records; a strict flasher/parser rejects or misreads them |
| 2 | Bare `#endif __ESI_EEPROM_H__` token (not a comment) | `generators/intel_hex.rs:87` | `binaries.js:89` | ✅ **FIXED** | Was: warns under `-Wextra`/`-pedantic`. Now emits `#endif /* … */` |
| 3 | VISIBLE_STRING `value` column always `"0"` placeholder | `generators/objectlist.rs:162-167` | `objectlist.js:183-196` | 🟢 By design | None — string default travels via `utypes`/`data`, not the value column |
| 4 | SM2/SM3 physical **size** hard-written `0` in the SII | `generators/eeprom.rs` `write_sync_managers` | `EEPROM.js:252,261` | 🟡 Quirk (likely fine) | Normal in many SIIs; master derives size from PDO config. Not validated |
| 5 | `MAX_{RX,TX}PDO_SIZE = 512` hard-coded | `generators/ecat_options.rs:45` | `ecat_options.js:58-59` | 🟡 Limitation | Over-allocates buffer; only wrong if a real PDO exceeds 512 bytes (rare) |
| 6 | BOOLEAN counts as **2** mappings; "array of booleans" unhandled | `generators/ecat_options.rs` `get_max_mappings` | `ecat_options.js:74-77,85-87` | 🟢 single / 🟡 array | Correct for a single BOOLEAN; a BOOLEAN *array* is a reference TODO (unsupported) |
| 7 | DC opmode `NaN`-is-truthy coercion for cycle/shift times | `generators/esi.rs:548-549` | `esi_xml.js:306-315` | 🟡 Quirk | Only bites malformed (non-numeric/whitespace) DC input; numeric input is exact |

Details below. All line numbers are current as of the merge to `master` (`0bb392a`).

---

## 1. 🔴 Intel-HEX checksum is not zero-padded

**Reference (`binaries.js`, `toIntelHex`):**
```js
// data bytes ARE padded to 2 chars:
let byte = record[byteposition].toString(16).slice(-2);   // line 39
if (byte.length < 2) byte = '0' + byte;                   // line 40-41  ✅ padded
...
// but the checksum is NOT:
checksum %= 0x100;
checksum = 0x100 - checksum;                              // two's complement
rule += checksum.toString(16).slice(-2) + '\n';           // line 52     ❌ no pad
```

The Intel-HEX format requires **every** byte field to be exactly two hex digits.
The reference pads data bytes (line 40-41) but forgot to pad the checksum (line 52).
So when the two's-complement checksum lands in `0x01..=0x0F`, `toString(16)` is a
single character and `.slice(-2)` leaves it a single character — the record's checksum
field is one nibble wide instead of two, and the line is malformed.

**What we did:** replicated it exactly (`intel_hex.rs:42-49`) — including correctly
reproducing the `checksum == 0x100` edge (`"100".slice(-2) == "00"`, a two-digit case
that a naive `% 0x100` would break; see the Task 13 fix). The single-digit case is
preserved because the golden `eeprom.hex` contains such records (e.g. the record at
address `0x0100`).

**Is it really a bug?** Yes, unambiguously. The `.hex` is only *coincidentally*
readable — a checksum in `0x01..=0x0F` (~1 in 16 records) produces a line that a
compliant parser (`srec_cat`, `objcopy`, most EEPROM flashers) would reject or
misparse. For a 2048-byte image (~64 data records) expect a few malformed records per
file. It went unnoticed upstream presumably because consumers use the `.bin`, not the
`.hex`.

**Recommendation:** worth fixing — zero-pad the checksum to two digits (a two-line
change: pad the `full` slice). **This changes the `eeprom.hex` golden**, so it's a
deliberate divergence like bugs 1/4/5, not a silent edit. The `.bin` is unaffected.
Decision needed: does anyone consume the `.hex`? If yes → fix. If the `.bin` is the
only artifact anyone flashes → safe to leave, but document it as known-broken.

---

## 2. ✅ Bare `#endif __ESI_EEPROM_H__` token — **FIXED**

**Reference (`binaries.js:89`):**
```c
};
#endif __ESI_EEPROM_H__
```
Tokens after `#endif` are ignored by the C preprocessor but are non-conformant —
GCC/Clang accept it, `-Wextra`/`-pedantic` warn "extra tokens at end of #endif
directive."

**What we did:** **fixed it.** `to_esi_eeprom_h` (`intel_hex.rs:87`) now emits the
conformant comment form:
```c
};
#endif /* __ESI_EEPROM_H__ */
```
This is a 4th intentional divergence from the reference. The three `eeprom.h` goldens
were regenerated to match — `scripts/dump_golden.js` applies the substitution in
`patchDivergences()` (alongside the bug-4 Physics patch), so re-running the dump
reproduces the corrected goldens instead of reverting the fix. Verified: regeneration
changed *only* the three `eeprom.h` files, nothing else drifted.

**Recommendation:** done. See the divergence appendix below.

---

## 3. 🟢 VISIBLE_STRING value column is always `"0"`

**Reference (`objectlist.js:183-196`, `objectlist_getItemValue`):**
```js
let value = '0';
if (objd.value) {
    switch(dtype) {
        case DTYPE.REAL32:       return `0x${float32ToHex(value)}`;  // ← bug-1 (FIXED in port)
        case DTYPE.VISIBLE_STRING: return value;                     // ← returns the '0' placeholder
        default:                 return `${objd.value}`;
    }
}
return value;
```
The VISIBLE_STRING branch returns the placeholder `'0'`, never the actual string.
Ours does the same (`objectlist.rs:162-167`).

**Is it really a bug?** No — by design. In the generated `objectlist.c`, a string
object's default *value* is not carried in the OBJD value column; the string's storage
and default live in the `utypes` struct / the `data` linkage. The value column just
needs a well-formed placeholder. The reference's own specs pin `'0'` here and pass.

*(Note: this shares a source line with bug-1. Bug-1 is the REAL32 branch reading the
`'0'` placeholder instead of `objd.value` — that one **is** a bug and we **fixed** it.
The VISIBLE_STRING branch returning `'0'` is the intentional part. Same function, two
different verdicts.)*

**Recommendation:** keep. If string *default values* ever need to round-trip into the
C, that's a utypes-initialization feature, not an objectlist value-column change.

---

## 4. 🟡 SM2/SM3 physical size written as `0`

**Reference (`EEPROM.js`, `writeSyncManagers`):**
```js
// SM0/SM1 (mailbox) — real size:
writeEEPROMword_wordaddress(parseInt(form.MailboxSize.value), offset/2, record);  // 234, 243
// SM2/SM3 (process data) — hard 0:
writeEEPROMword_wordaddress(0, offset/2, record); // 252  SM2 physical size
writeEEPROMword_wordaddress(0, offset/2, record); // 261  SM3 physical size
```
The mailbox SyncManagers get their real length; the process-data SyncManagers (SM2/SM3)
get length `0`, never computed from the actual PDO byte-length. Ours matches (the
full-image `eeprom.bin` golden passes, so the `0`s are reproduced).

**Is it really a bug?** Probably not. Per ETG.1000, an SII SyncManager length of `0`
for process-data SMs is a legitimate "let the master size it from the PDO assignment"
convention, and many real SII files do exactly this. It's a **gap** only in that the
value is never validated against the actual mapped PDO length — if a master required a
concrete size here, this would under-specify it.

**Recommendation:** leave (matches reference and common practice). If a specific master
rejects size-0 process SMs, compute it from the PDO mapping length. Already noted in
`bugs-ledger.md`.

---

## 5. 🟡 `MAX_{RX,TX}PDO_SIZE = 512` hard-coded

**Reference (`ecat_options.js:58-59`):**
```js
ecat_options += '#define MAX_RXPDO_SIZE   512'   // TODO calculate based on offset, size
             +  '\n#define MAX_TXPDO_SIZE   512\n\n'
```
A fixed 512-byte upper bound, with the reference's own `// TODO`. Ours emits the same
constant (`ecat_options.rs:45`).

**Is it really a bug?** No — a conservative limitation. 512 bytes comfortably exceeds
typical PDO sizes, so it only over-allocates the SOES PDO buffers slightly. It would be
*wrong* only if a device's actual PDO data exceeded 512 bytes, which is unusual for the
single-non-dynamic-PDO-per-direction devices this tool targets.

**Recommendation:** leave. Compute from the real PDO length only if tight RAM on the
target ever makes the over-allocation matter.

---

## 6. 🟢/🟡 BOOLEAN counts as two mappings; boolean *arrays* unhandled

**Reference (`ecat_options.js`, `getMaxMappings`):**
```js
++result;
if (subitem.dtype == DTYPE.BOOLEAN) {
    ++result;                 // boolean padding is mapping too      (74-77, array/record loop)
    // TODO handle array of booleans
}
...
++result;
if (objd.dtype == DTYPE.BOOLEAN) { ++result; }                       // 85-87, VAR branch
```
A mapped BOOLEAN counts as 2 entries — the data entry **plus** a 7-bit padding entry.
This is *correct*: `od_build` synthesizes exactly that data-entry + 7-bit-padding pair
in the PDO mapping record, so `MAX_MAPPINGS = 2` matches reality. Our port replicates it
and the dedicated Task-10 test pins `#define MAX_MAPPINGS_SM3 2` for one BOOLEAN.

**Is it really a bug?** For a **single** BOOLEAN: no, correct and tested. For an
**array of BOOLEANs**: the reference's own `// TODO handle array of booleans` admits the
counting is unconsidered — each subitem would add its own +1 padding, which is almost
certainly not how a packed boolean array should map. We faithfully match the reference's
limitation.

**Recommendation:** keep the single-BOOLEAN behavior (correct). Treat **boolean arrays
as unsupported** until the intended packing is specified — same stance as `bugs-ledger.md`.
A boolean-array project today produces a questionable `MAX_MAPPINGS`, matching the
reference; if boolean arrays become a requirement, this needs a real design, not a port
tweak.

---

## 7. 🟡 DC opmode `NaN`-is-truthy coercion

**Reference (`esi_xml.js:306-315`):**
```js
if (opMode.Sync0cycleTime && opMode.Sync0cycleTime != 0) { ...emit CycleTimeSync0... }
```
The field is emitted when it's truthy **and** `!= 0`. In JS, a non-numeric string
coerces through `Number()` for `!= 0`: `"abc" != 0` is `NaN != 0` → `true`, so a
non-numeric value is treated as *active* and emitted. An empty string is falsy → omitted.
We reproduce this (`esi.rs:548-549`): `!s.is_empty() && parse::<f64>().map(|n| n != 0.0).unwrap_or(true)`.

**Is it really a bug?** A quirk with essentially no real impact. DC cycle/shift-time
fields are numeric in every real project; a value of `0` is correctly omitted and a
real number is correctly emitted, identically in both. The only divergence (flagged in
Task 11 review) is a **whitespace-only** field: JS `Number(" ") == 0` → omitted, but our
`" ".parse::<f64>()` fails → `.unwrap_or(true)` → emitted. That requires deliberately
malformed DC input to hit.

**Recommendation:** leave. If we ever want strict DC validation, reject non-numeric
cycle/shift times outright (fail-loud) rather than matching the coercion — but that's a
divergence, not a bug fix.

---

## Appendix — where we *deliberately* diverged (for completeness)

So the fidelity picture is complete: these are **not** faithful copies — they are places
the port intentionally differs from the reference. The goldens either pin the corrected
output or are unaffected because no fixture exercises the path.

| Divergence | Reference behavior | Port behavior | Why |
|------------|--------------------|---------------|-----|
| Bug-1 REAL32 default | Emits `0x00000000` for every REAL32 (reads `'0'` placeholder) | IEEE-754 of `objd.value` (`1.5`→`0x3FC00000`) | Fix; golden regenerated |
| Bug-4 ESI Physics | `(P0+P1+P2) \|\| +P3` precedence drops Port3 | Concatenates all 4 ports; matches EEPROM `getPhysicalPort` | Fix; golden patched (`"YY "`→`"YY  "`) |
| `#endif` token (quirk #2) | Bare `#endif __ESI_EEPROM_H__` (non-conformant) | `#endif /* __ESI_EEPROM_H__ */` (conformant C) | Fix; `eeprom.h` goldens patched |
| Bug-5 EEPROMsize | Panics (reads past array) on bad/too-small size | Validates → `GenError::Config` (never panics) | Fix + trust-boundary hardening |
| Dtype-less OD object | Crashes (`ESI_DT[undefined]`) | `GenError::Od` (rejects at validation) | Trust-boundary fix (final review) |
| XML text/attrs | No escaping of `& < >` in manual XML build | `xml_escape` on all user text | Fix; no golden has `&<>`, so unaffected |
| Multiple PDO mappings/object | `alert()` then silently uses the first | `GenError` (fail-loud) | Reference limitation made explicit |
| Non-ASCII EEPROM strings | Truncates via `charCodeAt & 0xFF` (Latin-1) | `GenError` on `> 0x7F` (reject, don't mangle) | Trust-boundary; stricter |
| 64-bit bitsize | `dtype_bitsize` table stores `'64'` as a **string** | One numeric `ESI_DT.bitsize` (`u16`) everywhere | Internal cleanliness; output identical |
| `parse_u32` | JS `parseInt` is lenient (stops at first non-digit) | Strict (rejects trailing garbage) | Safer at trust boundary; all real fields are clean |

Plus three sane-handling choices on paths **no valid input reaches** (so no golden is
affected, and the reference would itself misbehave there): empty-PDO-section boolean
padding (JS `NaN` → we pass through), RECORD-with-VISIBLE_STRING-subitem PDO bitsize
(JS `NaN` → we `0`), and `sm_offset + N` u16 wrap for `>0xFFFF` PDO indexes.

---

## Bottom line

- **One real inherited bug to decide on:** #1, the Intel-HEX checksum padding. Fixable in
  two lines; the question is whether anyone consumes the `.hex` (if only the `.bin` is
  flashed, it's moot). Everything else is a quirk, a conservative limitation, or correct
  by design.
- **Two "match the reference limitation" stances** worth a conscious sign-off: #4 (SM2/SM3
  size 0) and #6 (boolean arrays) — both fine for the devices this tool targets, both
  gaps if the scope widens.
- **#2 is now fixed** (conformant `#endif /* … */`).
- The rest (#3, #5, #7) can stay as-is.
