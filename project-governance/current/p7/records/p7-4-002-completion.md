# P7.4-002 completion

The real Windows x86-64 `0.1.0` local archive is complete.

- It contains exactly eleven approved root files.
- It contains no database or preinstalled Rutgers data.
- `RBCSP.exe` embeds the local UI and uses the static MSVC runtime.
- A real extracted candidate created its package-relative database, persisted
  a synthetic selection, shut down cleanly, preserved data across root-file
  replacement, and recovered the selection through the packaged launcher.
- Two independent clean builds produced byte-identical ZIPs.

Gate: `P7_4_WINDOWS_LOCAL_ARCHIVE_PASS`.
