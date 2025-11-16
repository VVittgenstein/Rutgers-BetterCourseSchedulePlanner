# Rutgers SOC API Probe Notes

_Last updated: 2025-11-16_

## Probe CLI quickstart
- Install dependencies once with `npm install` (TypeScript + tsx runner are already configured).
- Run probes via `npm run soc:probe -- [flags]`. Required flags are `--term <semester>` and `--campus <code or comma list>`. Optional flags include `--subject <code>` (local filter + request context), `--endpoint courses|openSections`, `--samples <n>` and `--timeout <ms>`.
- Semesters accept multiple formats (`12024`, `20241`, `FA2024`). The script normalizes to SOC's `year` + `term` (0/1/7/9) parameters before calling `https://classes.rutgers.edu/soc/api/*`.
- Errors bubble up as JSON logs that include `requestId`, endpoint, HTTP status (when available) and a retry hint so failed experiments can be reproduced.

## Scenario snapshots (2024 data)
### 1. Spring 2024 • New Brunswick • Subject 198 (courses endpoint)
Command:
```bash
npm run soc:probe -- --term 12024 --campus NB --subject 198 --samples 2
```
Summary:
- Request ID `2ce8c3a5-b93b-4d10-a293-7a596419d6c6`, URL automatically expanded to `year=2024&term=1&campus=NB&subject=198`.
- Status `200 OK`, 195 ms end-to-end, decoded payload size ≈ 20.4 MB.
- Records returned: total campus payload 4,530 course objects, 66 match subject `198` locally.
- Sample excerpt:
  - `198-107 COMPUT MATH & SCIENC` (Busch campus, 2 sections, 2 open, 4 credits)
  - `198-110 PRINCIPLES OF CS` (Busch campus, 4 sections, 2 open, 3 credits)

### 2. Fall 2024 • Newark • Subject 640 (courses endpoint)
Command:
```bash
npm run soc:probe -- --term 92024 --campus NK --subject 640 --samples 2
```
Summary:
- Request ID `326816d2-727b-494a-803e-dde0892200cc`.
- Status `200 OK`, 274 ms, decoded payload ≈ 4.8 MB.
- Records: 1,286 Newark courses in total, 30 locally filtered rows for subject `640`.
- Sample excerpt:
  - `640-033 MATH LIB ARTS INTENS` (Newark campus, 1 section, 0 open, 3 credits)
  - `640-038 INTER ALGEBRA INT` (Newark campus, 18 sections, 4 open, 3 credits)

### 3. Summer 2024 • Camden • Open sections heartbeat
Command:
```bash
npm run soc:probe -- --term 72024 --campus CM --endpoint openSections --samples 5
```
Summary:
- Request ID `54ca2220-0c8e-45c2-b147-faa30a69735c`.
- Status `200 OK`, 140 ms, decoded payload ≈ 1.9 KB.
- Open section indexes returned: 238 total. Sample indexes `00622, 04667, 06151, 06152, 06146` confirm non-empty coverage for Camden summer data.

## Observations & follow-ups
- The public SOC endpoints ignore `subject` in the query string; filtering happens client-side (the probe mirrors this behavior and reports both total + filtered counts so downstream tasks know how much needs to be cached locally).
- Responses are served with `Content-Encoding: gzip`; `fetch` transparently decodes. Any custom HTTP client must send `Accept-Encoding` or handle gzip manually.
- Typical headers include `Cache-Control: max-age=900` and a persistent `ETag`, which hints that caching probes for at least 15 minutes will avoid redundant downloads.
- When the server returns non-2xx status codes, the probe emits structured JSON errors (`errorType`, `requestId`, `retryHint`). These logs can be copied into future tickets for reproducibility.
- Next experiments (field matrix & rate-limit tasks) can reuse the CLI or import the `performProbe` helper to fan out term/campus batches with controlled concurrency.
