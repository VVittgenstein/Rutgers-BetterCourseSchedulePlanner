# P7.4-002-R4 - Windows official hash lock

GitHub Actions Windows 2025 run `29407487411`, job `87326379450`, built frozen
source `7d5debef005277e4d8f2ed2b9fb2f72c495e62f1`. Its deterministic metadata
builder passed the exact 11-file package and printed SHA-256
`eabdc3b5f4a705d8c22e6941831f55e0bb5b5c2a1c33e648e545f86007cab577`.

The job then stopped at the deliberately retained old-hash comparison, before
standard-user acceptance. This is discovery evidence, not acceptance. Both
official platform hashes are now locked; the next run must complete every
Windows, Linux, product-flow, artifact and malware gate.
