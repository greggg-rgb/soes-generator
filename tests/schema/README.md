# EtherCAT ESI schemas (ETG.2000)

Vendored, unmodified, for offline schema-validation of the generated ESI XML
(see [`tests/esi_schema.rs`](../esi_schema.rs)).

- **Standard:** ETG.2000 v1.0.10 (EtherCAT Slave Information — the ESI/`EtherCATInfo`
  device-description schema published by the EtherCAT Technology Group).
- **Files:** `EtherCATInfo.xsd` is the entry point; it `xs:include`s `EtherCATBase.xsd`.
  `EtherCATDiag.xsd`, `EtherCATDict.xsd`, and `EtherCATModule.xsd` are the remaining
  members of the schema set (kept for completeness; `EtherCATInfo` pulls in what it needs).
- **Source of these copies:** the [DiamondLightSource/ethercat](https://github.com/DiamondLightSource/ethercat)
  mirror (`etc/xml/`), which redistributes the ETG schemas. They ship identically with
  Beckhoff TwinCAT and are the same files EtherCAT masters use to validate ESI.

These are third-party schema files reproduced verbatim; they are not covered by this
project's license.
