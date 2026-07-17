# P3 Open Round 2 Completion

## Outcome

- Status: `FROZEN_ROUND_2_COMPLETE`
- Completed at: `2026-07-13T04:21:45.011085Z`
- Attempts: `42/42` total; `21/21` Round 2; all HTTP 200 successes
- Terminal conflicts: `0`
- Further Rutgers requests for this P3 evidence run: `0 authorized`

The Round 2 amendment allowlist was exhausted without HTTP, shape, size, redirect, timeout, hash, or duplicate-value conflict. All 42 responses were arrays containing only unique five-digit strings. Two observations were legal empty arrays and both corresponded to empty Catalog scopes; no unsafe empty-with-nonempty-Catalog case occurred naturally.

## Offline findings

- Approved official intersection observations: `42/42 PASS`.
- Cross-round raw body changes: `14/21` scope pairs.
- Cross-round effective Catalog-section Open changes: `3/21` scope pairs.
- Effective changes summed non-distinct across scope pairs: `3 added`, `8 removed`.
- `Cache-Control: max-age=30`: `42/42`.
- `Date` and `ETag`: `42/42`; `Age` and `Last-Modified`: `0/42`.
- Request duration across the low-volume serial evidence run: median `1195.597 ms`, nearest-rank p95 `1501.020 ms`, max `1537.168 ms`.

These findings validate shape, paired transitions, and Rutgers-style set membership. They do not establish a Rutgers update SLA, hard end-to-end 30-second notification guarantee, or sustained production capacity.

## Frozen hashes

| Artifact | SHA-256 |
|---|---|
| Round 2 amendment | `4C3B9FA7FD1F7DE4DF35570A55D0479DE15B82F1D510CDBC7D34B9EA2F332E2B` |
| Final Open ledger | `DC7A44570C727AA3561A2468A361AB27191807E87A781E41EF81EFF730B7977E` |
| Evidence register | `6941261F92638FD9E108F0A169CBCA57103F51F1A4B8CD9D6B21A1DF5B718B9B` |
| Open profile | `C35F8CEA09D3D29BD5781E3721B882E0A6A58989E8D929782E0D8956ADB8522C` |
| Scope intersection table | `0D12286FDF2317F1822DCA7382204BB5C8E57B1B398F74D95E740366C2F916D9` |
| Body clusters | `31FBA85B6C46DD9012A22614DA8391D11B92D48993FA6ACEEFCD01B85DBF5AA8` |
| Transition table | `6354CDC97A9CE6501110DD08A20A6E33F70BAA7A97EF62D1B7A445C6B70B3BB8` |
| Value-shape table | `3E4D9371E8DE8F074162D3459CBCA0643C028264D9C11BBFC7FF7CA1A27C908F` |

