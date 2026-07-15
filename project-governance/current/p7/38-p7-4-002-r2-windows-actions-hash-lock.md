# P7.4-002-R2 - Windows Actions candidate hash lock

The first official-runner build for repaired source
`476565cbe8e19075214cdc1427c86cf2dcf4e966` completed the exact 11-file package
verifier and produced SHA-256
`2c807e475779f9d3f7e8549921655dad87c4dff3b06286d05ac2a985fa2907d2`.

Run `29399422048` then stopped at the retained local-build hash comparison,
before standard-user acceptance. It is discovery evidence only. This task locks
the official Windows runner hash; the next push run must pass every package,
product-flow, artifact, and malware job before it can count as acceptance.

Next task: final push-run and manual-dispatch acceptance.
