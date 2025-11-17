# Rutgers SOC Rate-limit Profile

_Last updated: 2025-11-17 (term 12024, campus NB baseline). Raw metrics are saved in `docs/soc_rate_limit.latest.json` plus stress captures under the same folder._

## Runner setup
- Tooling: `npm run soc:rate-limit -- --term 12024 --campus NB --subject 198 --endpoint both --schedule 1:1200,3:600,6:300 --iterations 20 --rest 4000 --output docs/soc_rate_limit.latest.json`.
- Each scenario fires N requests with the specified worker count (`concurrency`) and inter-request gap (`intervalMs`). `estimated RPS` (workers × 1000/interval) shows the theoretical max while `actual RPS` divides successful calls by end-to-end duration.
- Stress notes: extra sweeps pushed `courses.json` to 32 workers/50 ms gaps (file `soc_rate_limit.courses_stress2.json`) and `openSections` to 20–50 workers with zero gaps (file `soc_rate_limit.openSections_blitz.json` plus console output of the 500-request burst).

## Baseline results

### courses.json (New Brunswick spring 2024 payload ≈20 MB gzip decoded)

| Concurrency × gap | Theoretical RPS | Actual RPS | 2xx / 4xx / 5xx | Avg latency (ms) | p95 latency (ms) | Notes |
| --- | --- | --- | --- | ---:| ---:| --- |
| 1 × 1200 ms | 0.83 | 0.70 | 20 / 0 / 0 | 166 | 978 | Single worker saturates around 0.7 req/s because each payload is ~20 MB and gzip decode dominates. |
| 3 × 600 ms | 5.00 | 3.39 | 20 / 0 / 0 | 177 | 943 | Sweet spot for steady fetch: 3.4 req/s keeps CPU <30% on laptop; six term+campus pulls finish in ≈6 s. |
| 6 × 300 ms | 20.00 | 7.60 | 20 / 0 / 0 | 300 | 674 | More workers do not improve throughput proportionally (bandwidth limited) and increase memory pressure. |
| 12 × 150 ms (stress) | 80.00 | 10.05 | 32 / 0 / 0 | 803 | 1,106 | Still 100% 2xx but per-request latency jumps above 0.8 s; keep for emergency replays only. |
| 32 × 50 ms (stress) | 640.00 | 10.81 | 40 / 0 / 0 | 1,884 | 2,877 | No 429/5xx observed, yet download throughput exceeds 1.8 GB/min; impractical for commodity machines. |

### openSections.json (flat index list, ≤10 KB per response)

| Concurrency × gap | Theoretical RPS | Actual RPS | 2xx / 4xx / 5xx | Avg latency (ms) | p95 latency (ms) | Notes |
| --- | --- | --- | --- | ---:| ---:| --- |
| 1 × 1200 ms | 0.83 | 0.80 | 20 / 0 / 0 | 44 | 86 | Open-section heartbeat is tiny; even a single worker stays <100 ms latency. |
| 3 × 600 ms | 5.00 | 4.41 | 20 / 0 / 0 | 41 | 80 | Good default for polling loops; aligns with 15 min cache headers. |
| 6 × 300 ms | 20.00 | 14.56 | 20 / 0 / 0 | 51 | 168 | No errors and latency remains sub-200 ms. |
| 20 × 0 ms (stress) | ∞ | 50.06 | 120 / 0 / 0 | 322 | 1,939 | Short 120-request burst with zero delay still hit 100% 2xx; jitter spikes only happen when queued behind slow TLS handshakes. |
| 50 × 0 ms (console) | ∞ | 586.98 | 500 / 0 / 0 | 71 | 201 | 500 immediate requests completed in 0.85 s without throttling; we still cap production to avoid being noisy neighbors. |

## Observations
- SOC backend appears bandwidth-bound rather than request-count bound for `courses.json`. Throughput plateaus near 10 req/s regardless of worker count because each response delivers 20 MB+ of JSON plus gzip inflate time.
- `openSections` can sustain 50+ req/s bursts without Surfaced throttling, suggesting the upstream either lacks rate limiting or the quota is very high. To remain polite, we will impose our own caps (<25 req/s steady, <50 req/s bursts).
- None of the sweeps returned 4xx/5xx, so there is no hard limit within the tested window. However, Rutgers previously flipped CDN rules without notice (ref. `R-20251113-soc-api-change`), so retaining aggressive backoff logic is still necessary.

## Recommended operating profile

### courses.json (full refresh + incremental)
1. **Full refresh**: batch `term × campus` combos in groups of three workers, 600 ms gap (`≈3.4 req/s`). Six combos complete in ~6 s; for multi-term environments double the runtime linearly.
2. **Incremental refresh**: run single worker with 1.2 s gap when only one campus changes; this keeps outbound volume under 20 MB/s and avoids clobbering laptops.
3. **Backoff plan**: on first retryable error drop concurrency to one worker and double the interval (1.2 s → 2.4 s → 4.8 s) with full jitter. Resume the nominal profile only after 5 consecutive successes.

### openSections.json (polling / alert loop)
1. **Continuous polling**: 10 concurrent workers, 250 ms gap (`≈40 req/s theoretical`, ~30 req/s in practice) keeps the per-campus heartbeat <1 s while staying conservative.
2. **Burst catch-up**: up to 20 workers with zero gap for at most 200 requests (≈50 req/s) is safe for manual catch-ups; beyond that throttle down gradually.
3. **Backoff plan**: on any non-2xx response, pause for 15 s, resume at 5 workers / 500 ms gap, then ramp up 2 workers per minute until stable.

## Error handling playbook

| Code / signal | Trigger & notes | Recommended action |
| --- | --- | --- |
| 429 Too Many Requests | CDN applies when sustained bursts exceed ~60 req/s per IP (seen historically though not reproduced in this sweep). | Immediately pause 60 s, clear inflight queue, relaunch at `1×1200 ms` for courses or `5×500 ms` for openSections; only ramp after 5 green responses. |
| 503 / 504 Service Unavailable | Usually appears during nightly SOC maintenance or when Rutgers flips backend deployments. | Retry with exponential backoff starting at 30 s (courses) / 10 s (openSections). Log `requestId` from CLI output to correlate with upstream incidents. |
| 400 / 404 | Invalid `term`/`campus` params or when a new term is not yet published. | Treat as configuration bugs: stop the batch, alert operator, and do not retry until parameters are corrected. |
| Client TIMEOUT | Our `timeoutMs` (default 15 s) fired because payload download stalled. Often coincides with Wi-Fi/VPN drops. | Count as soft failure, retry once after 5 s; if it recurs 3× in 5 min drop concurrency by half and surface an alert. |
| Network / DNS errors | Node fetch cannot resolve the SOC host (rare but happens on flaky VPN). | Switch to backup network path or disable VPN, then resume with the low-frequency profile. |

### Reference scripts
- `scripts/soc_rate_limit.ts`: orchestrates the scenarios above and emits JSON/console summaries.
- `docs/soc_rate_limit.latest.json`: canonical raw results for the 2025-11-17 sweep; can be re-run with fresh `--term`/`--campus` pairs when Rutgers updates data.
