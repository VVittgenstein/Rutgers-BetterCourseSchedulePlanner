# P7.4-001 completion

The two `0.1.0` release inputs are frozen after full local integration
verification.

- Real local and public UI are embedded in their Rust runtimes.
- Windows remains first-run, package-relative, and free of preinstalled course
  data.
- Public delivery no longer requires an external UI directory.
- Rust, React, fake-upstream, browser, architecture, license, and release-root
  checks pass.
- Exactly two package filenames and their allowlists/denylists are fixed in
  `packaging/release-inputs.json`.

No package artifact or production mutation occurred. After the exact pushed
commit passes the Ubuntu operations rehearsal, work continues directly with
the Windows and Linux package tasks.

Gate: `P7_4_RELEASE_INPUTS_FROZEN_LOCAL_PASS`.
