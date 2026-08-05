# soes_generator — engineering docs

Rust port of [kubabuda/EEPROM_generator](https://github.com/kubabuda/EEPROM_generator)
(EtherCAT slave EEPROM / [SOES](https://github.com/OpenEtherCATsociety/SOES) code generator).
Reference cloned at `.cache/EEPROM_generator` (commit `6f26c1b`, gitignored).

## Start here

- [`../README.md`](../README.md) — crate README: `build.rs` codegen, CLI usage, the
  `generate`/`emit` API, and the kept reference limitations.
- [`00-overview.md`](00-overview.md) — architecture, inputs/outputs, code flow, the hard parts.
- [`feasibility-and-port-plan.md`](feasibility-and-port-plan.md) — verdict (feasible, low-risk),
  proposed Rust crate shape, API sketch, test strategy, sequencing.
- [`bugs-ledger.md`](bugs-ledger.md) — 5 confirmed bugs (with proofs) + latent issues, each with
  a port decision.

## Deep-dive research (source-of-truth for the port)

- [`research/01-domain-model-and-flow.md`](research/01-domain-model-and-flow.md) — the OD data
  model, the ~31-field config schema, constants/enums to port exactly, validation rules.
- [`research/02-generators.md`](research/02-generators.md) — all 5 generators: output format,
  algorithm, byte/bit math, fragile parts.
- [`research/03-binaries-io-readers.md`](research/03-binaries-io-readers.md) — SII byte layout,
  CRC, Intel-HEX, file bundling, and why round-trip goes through JSON not ESI-XML.
- [`research/04-tests-and-bugs.md`](research/04-tests-and-bugs.md) — test-suite map, reusable
  golden vectors, coverage gaps, and the bug hunt.
- [`research/repro/`](research/repro/) — headless harness + bug-proof scripts (73-spec oracle).

## Regenerating the reference oracle

```bash
cd .cache/EEPROM_generator && npm install jsdom
NODE_PATH="$(pwd)/node_modules" node ../../docs/research/repro/harness.js   # → 73 specs pass
```
