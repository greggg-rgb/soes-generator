# Reference-behavior harness & bug proofs

These scripts drive the **JavaScript reference** (`.cache/EEPROM_generator/`, pinned at
commit `6f26c1b`) headlessly via `jsdom`. They exist to (a) run the original Jasmine
suite as a live oracle for the Rust port, and (b) prove the 5 confirmed bugs are real.

## Prerequisites

```bash
cd .cache/EEPROM_generator && npm install jsdom     # one-time; node_modules is gitignored
```

## Run

```bash
cd .cache/EEPROM_generator
NODE_PATH="$(pwd)/node_modules" node ../../docs/research/repro/harness.js   # full Jasmine suite → 73 specs, all pass
NODE_PATH="$(pwd)/node_modules" node ../../docs/research/repro/prove.js     # bugs 1–4
NODE_PATH="$(pwd)/node_modules" node ../../docs/research/repro/prove2.js    # bug 5 (Intel-hex odd size)
```

## Expected output (verified 2026-08-05)

- `harness.js` → `TOTAL SPECS: 73 OVERALL: passed`, `FAILURES: 0`
- `prove.js`:
  - `real32_line` = `...DTYPE_REAL32, 32, ATYPE_RO, acName2000, 0x00000000, NULL` — BUG 1 (should be `0x3FC00000` for 1.5)
  - `physics` = `"YYY"` — BUG 4 (Port3=`H` dropped)
  - `reader_UploadAtStartup_wrongKey` = `true`, correct key absent — BUG 2
  - `reader_emptyType_err` = `TypeError: ...getElementsByTagGroupType is not a function` — BUG 3
- `prove2.js`:
  - `hex2000_err` = `TypeError: Cannot read properties of undefined` — BUG 5 (`EEPROMsize=2000`)
  - `hex2048` = `OK`

See [`../../bugs-ledger.md`](../../bugs-ledger.md) for the full analysis and the port decision per bug.

The generators' Jasmine `toEqualLines` expected strings (default / CiA-402 / FoE / per-dtype
projects + 6 ESC config-data strings) are the **golden vectors** the Rust port should be diffed
against — see [`../04-tests-and-bugs.md`](../04-tests-and-bugs.md) §Fixtures.
