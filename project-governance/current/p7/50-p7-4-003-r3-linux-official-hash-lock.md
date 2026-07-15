# P7.4-003-R3 - Linux official hash lock

GitHub Actions Ubuntu 24.04 run `29407487411`, job `87326379524`, built frozen
source `7d5debef005277e4d8f2ed2b9fb2f72c495e62f1`. The deterministic builder and
package verifier passed the exact 20-file package, then printed SHA-256
`9bebd35808497e40ae36cb459c681ffb2ffe29c3b824988e460745d29f03605d`.

The job then stopped at the deliberately retained old-hash comparison. This is
discovery evidence, not acceptance. The new Linux hash and cache key are now
locked; the next complete run must pass installation, Caddy and browser gates.
