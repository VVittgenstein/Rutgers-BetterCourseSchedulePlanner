# P7.4-002-R1 - Windows replacement candidate

- Frozen source: `476565cbe8e19075214cdc1427c86cf2dcf4e966`
- Archive: `rbcsp-windows-x86_64-0.1.0.zip`
- Files: exactly `11`
- Bytes: `5563981`
- SHA-256: `6118cde5db6efad68be11c907671b76bd83d831e8ad93567bbc20cd4c9c6fe30`

The replacement local package embeds the repaired Local UI and runtime in the
real `RBCSP.exe`. It contains the existing exact allowlist and no `data`
directory, database, seed, or real Rutgers data. First start remains responsible
for package-relative `data/rbcsp.sqlite` creation.

Two independent build/output roots produced byte-identical archives. The real
archive verifier passed its file, provenance, hash, static-runtime, first-start,
restart, state-preservation, shutdown, and two-scenario browser checks with the
exact no-Rutgers CI switch. This fake-upstream browser verification supplements,
but does not replace, the later Chrome-use real-world E2E.

The archive remains ignored under `release/0.1.0`; it is not committed. Every
earlier Windows candidate remains ineligible.

Gate: `P7_4_WINDOWS_REPLACEMENT_CANDIDATE_PASS`.
Next task: `P7.4-003-R1`.
