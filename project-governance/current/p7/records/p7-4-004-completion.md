# P7.4-004 completion

The two real `0.1.0` release candidates pass their joint static audit.

- The release directory contains exactly the Windows ZIP and Linux tarball.
- Their exact `11`/`20` file contracts, metadata, hashes, licenses, notices,
  provenance, target binaries, SBOM graphs, and embedded frontend closure pass.
- No database, preloaded course data, secret, private build path, test/evidence
  residue, or platform-crossing surface is present.
- Two independent builds of each package are byte-identical.

Gate: `P7_4_CROSS_PACKAGE_AUDIT_PASS`.
Next: `P7.4-005` candidate acceptance and P7.5 entry gate.
