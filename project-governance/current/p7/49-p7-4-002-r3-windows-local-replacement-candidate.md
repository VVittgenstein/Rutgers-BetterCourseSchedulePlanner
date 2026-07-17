# P7.4-002-R3 - Windows local replacement candidate

- Frozen source: `7d5debef005277e4d8f2ed2b9fb2f72c495e62f1`
- Archive: `rbcsp-windows-x86_64-0.1.0.zip`
- Files: exactly `11`
- Bytes: `5574298`
- Local SHA-256: `02ec41a869b3c53f8682dd69b8e7f2a53fb1a26703eb1f147ecbc11dac1f174a`

Two clean detached source worktrees and separate build/output roots produced
byte-identical archives. Each archive passed the real package verifier,
including allowlist/provenance/static-runtime checks, first start, restart,
state preservation, shutdown and two fake-upstream browser scenarios. No
database or Rutgers data is preinstalled.

The local MSVC result is reproducibility evidence, not the canonical candidate.
The official Windows runner hash remains pending.
