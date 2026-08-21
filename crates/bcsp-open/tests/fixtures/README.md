# Gate replay fixtures — real captured Rutgers data

Decoded `openSections.json` index arrays captured live on 2026-08-19 during
the verified partial-snapshot incident (NB, term 92026), extracted from the
evidence archive `data/open-sections-repro/20260819T2117Z-original-capture/`
(not git-tracked; independently verified, see the verification directory
next to it).

| fixture | cardinality | source file | source SHA-256 |
|---|---|---|---|
| `pre_anomaly_full.json` | 11,423 | `c_2116.txt` (server 21:16:16 UTC) | `728ac73b5b08364ba7a00a9b75e929b8edcb2723d04ebf33f5ae204859251553` |
| `partial.json` | 8,146 | `r_0.json` (21:17:16; r_0..r_12 byte-identical) | `703d7a927cdcb1bde8603cefd5c4546cf4a0cc8397ed93e7c0671304bb0ae40d` |
| `recovery_full.json` | 11,422 | `r_13.json` (21:18:01) | `5367c629b57707cab6d484175e40e680fbce0e6eef88dfe7231805a9f04f7ffd` |

All thirteen partial samples in the incident were byte-identical, so one
partial fixture replayed thirteen times reproduces the full anomaly timeline
(>= 41 s proven duration, ~105 s outer envelope, recovery at a minute
boundary). `pre_anomaly_full.json` was converted from the archive's
newline-separated list to a JSON array; the array contents are unchanged.
