# P7.4-005-R1 - Independent runner replay

The GitHub web UI cannot dispatch this branch-only workflow because the approved
isolation keeps it out of the default branch. This no-product-change commit
triggers a second independent push run after both official-runner hashes were
locked. It must rebuild the same two candidate bytes and pass every Windows,
Linux, product-flow, artifact, and malware gate.

First accepted run: `29400462225`.
Next task: second-run acceptance and byte comparison.
