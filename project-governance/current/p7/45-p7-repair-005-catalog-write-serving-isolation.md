# P7-REPAIR-005 - Catalog write/serving isolation

The approved Windows candidate retry proved that the browser was waiting on a
runtime which could not serve bootstrap while a large Catalog publication was
in progress. The candidate was stopped, cleaned and retired.

This repair keeps the single package-relative/public state database and makes
its concurrency explicit:

- every file-backed operational database enables and verifies SQLite WAL;
- Local and Public use separate refresh-writer and serving connections to the
  same database file;
- Public product routes, status and watch admission use the serving connection;
- Catalog normalization, mapping and publication run on Tokio's blocking pool;
- local personal writes may wait up to 120 seconds for SQLite's single writer.

Regression tests hold real writer locks and prove that Local bootstrap,
Catalog discovery, Public documents and readiness remain bounded. The local
personal-write test also proves waiting beyond the former five-second limit.

Acceptance: Rust formatting, all workspace/all-target tests, and Clippy with
warnings denied pass offline. No real Rutgers response data is stored.

Next task: deterministic frontend gate repair, then source refreeze and new
Windows/Linux candidates.
