## Subtask
- ID: ST-20251113-soc-api-validation-01-probe
- Scope: build a minimal Rutgers SOC probe CLI that unifies `courses.json` / `openSections` calls, surfaces request metrics, and records sample outputs for ≥3 term/campus combos.

## Confirmed Implementation Facts
1. Added npm/TypeScript tooling (`package.json`, `package-lock.json`, `tsconfig.json`) with `npm run soc:probe` wired to execute `scripts/soc_probe.ts` via `tsx`.
2. `scripts/soc_probe.ts` parses CLI flags (`term`, `campus`, `subject`, `endpoint`, `samples`, `timeout`, `level`), normalizes semester inputs into SOC `year` + `term` parameters, issues a single fetch to `https://classes.rutgers.edu/soc/api/<endpoint>.json`, and prints status code, latency, decoded size, and filtered sample rows.
3. The probe logs structured JSON errors (request id, endpoint, retry hint) for HTTP errors, JSON parse failures, network issues, and timeouts before exiting non-zero.
4. `docs/soc_api_notes.md` documents how to run the CLI plus three recorded scenarios (Spring24 NB subject 198, Fall24 NK subject 640, Summer24 CM openSections) with request ids, dataset sizes, and sample outputs.
5. `record.json` marks this subtask as `done`, clears blockers, and sets `updated_at` to `2025-11-16T00:00:00Z` for traceability.

## Interface / Behavior Changes
- New CLI command `npm run soc:probe` is now part of the developer workflow; it accepts Rutgers semester aliases (`12024`, `FA2024`, etc.), emits human-readable summaries, and can be reused by downstream scripts for multi-term probing.
- New artifact `docs/soc_api_notes.md` serves as the canonical reference for SOC probe usage and captured samples.

## Risks / Limits / TODOs
- The probe currently runs a single HTTP request per invocation; batching/concurrency control for larger sweeps remains TBD.
- Subject filtering is performed only client-side (per SOC behavior); no server-side filtering guarantees beyond what the API returns.
- Rate-limit profiling and wider field coverage are explicitly deferred to subtasks ST-20251113-soc-api-validation-02-field-matrix and ST-20251113-soc-api-validation-03-limit-profile.

## Self-tests
- `npm run soc:probe -- --term 12024 --campus NB --subject 198 --samples 2` (courses endpoint, success)
- `npm run soc:probe -- --term 92024 --campus NK --subject 640 --samples 2` (courses endpoint, success)
- `npm run soc:probe -- --term 72024 --campus CM --endpoint openSections --samples 5` (openSections endpoint, success)

## Code Review - ST-20251113-soc-api-validation-01-probe - 2025-11-16T13:45:18Z
Codex Review
Here are some automated review suggestions for this pull request.

ℹ️ About Codex in GitHub
scripts/soc_probe.ts
Comment on lines +245 to +303
    if (!response.ok) {
      emitStructuredError({
        requestId,
        endpoint: options.endpoint,
        url,
        httpStatus: response.status,
        statusText: response.statusText,
        retryHint: deriveRetryHint(response.status),
        errorType: 'HTTP',
        detail: buffer.toString('utf-8').slice(0, 400)
      });
      throw new Error(`Request failed with status ${response.status}`);
    }

    let body: unknown;
    try {
      body = JSON.parse(buffer.toString('utf-8'));
    } catch (error) {
      emitStructuredError({
        requestId,
        endpoint: options.endpoint,
        url,
        httpStatus: response.status,
        statusText: response.statusText,
        retryHint: 'Inspect response payload, JSON parse failed',
        errorType: 'JSON_PARSE',
        detail: (error as Error).message
      });
      throw error;
    }

    return {
      requestId,
      url,
      statusCode: response.status,
      statusText: response.statusText,
      durationMs,
      sizeBytes,
      body
    };
  } catch (error) {
    if ((error as Error).name === 'AbortError') {
      emitStructuredError({
        requestId,
        endpoint: options.endpoint,
        url,
        retryHint: 'Request timed out. Increase --timeout or lower concurrency.',
        errorType: 'TIMEOUT',
        detail: 'AbortError triggered by timeout'
      });
    } else if (!(error instanceof Error && error.message.startsWith('Request failed'))) {
      emitStructuredError({
        requestId,
        endpoint: options.endpoint,
        url,
        retryHint: 'Check network connectivity or VPN settings.',
        errorType: 'NETWORK',
        detail: (error as Error).message
      });
@chatgpt-codex-connector
chatgpt-codex-connector bot
1 minute ago
P2 Badge Avoid double logging when JSON parse fails

If the API returns non‑JSON data, performProbe emits a structured error in the inner catch with errorType: 'JSON_PARSE', but the outer catch immediately logs a second entry tagged as NETWORK. The duplicate log is misleading because the second message overwrites the original context and suggests a connectivity issue rather than a payload problem. Consider short‑circuiting the outer handler for errors that have already been logged or passing an explicit flag so JSON parse failures do not produce two conflicting error entries.
