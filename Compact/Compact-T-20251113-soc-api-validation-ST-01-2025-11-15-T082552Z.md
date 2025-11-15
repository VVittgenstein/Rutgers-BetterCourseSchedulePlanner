## Confirmed Facts
- Added `docs/soc_api_map.md` summarizing Rutgers SOC resources (`courses.json`, `openSections.json`, legacy `sections.json` unusable, and inline `initJsonData`) with required params, sample payload sizes, and gzip behavior verified via live Fall 2024 NB & Spring 2025 NK calls.
- Verified only `year` + `term` + `campus` affect SOC API responses; `subject`/`keyword`/`level`/`school` query params are ignored server-side, so filtering must be client-side; case-sensitive campus codes and invalid term/campus yield 200 + empty arrays.
- Documented course/section/meetingTime schema (fields like `creditsObject`, `coreCodes`, section `openStatus`, meeting mode codes, etc.) plus relation between `openSections.json` index list and `sections[*].index` for availability tracking.
- Captured “official vs actual” discrepancies (e.g., missing `sections.json`, ineffective `level` param) and proposed operational strategy: term+campus batching, caching `initJsonData`, using `openSections.json` for polling, and logging HTTP 400 vs empty responses.

## Interfaces / Behavior Changes
- No runtime code touched; new documentation artifact adds authoritative contract for SOC data ingestion modules to consume.

## Risks / TODO
- Rate limiting characteristics remain unknown; further high-frequency testing pending future subtasks.
- `docs/soc_api_map.md` is manually curated; needs periodic refresh if Rutgers front-end JS (`soc_utils.js`) changes constant tables.

## Code Review - T-20251113-soc-api-validation-ST-01 - 2025-11-15T09:24:05Z
Codex Review: Didn't find any major issues. 🚀
