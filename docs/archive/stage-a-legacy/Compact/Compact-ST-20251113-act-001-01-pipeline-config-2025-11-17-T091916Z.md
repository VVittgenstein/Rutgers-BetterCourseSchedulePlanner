### Subtask ST-20251113-act-001-01-pipeline-config

**Confirmed facts**
- `package.json` exposes a real `npm run data:fetch` entry wired to the new `scripts/fetch_soc_data.ts` stub, which parses all documented flags, validates the JSON config, and prints the resolved term/campus/subject plan without touching the network/DB yet.
- `docs/fetch_pipeline.md` now clarifies that the command is a planning stub until ST-20251113-act-001-02 ships, and points editors to the dedicated schema file.
- `configs/fetch_pipeline.schema.json` describes the config contract (`concurrency`, `retryPolicy`, `targets`, mode-specific blocks, safety/summary sections) and allows disabling the openSections poller by permitting `maxOpenSectionsWorkers = 0` (and the downgraded profile equivalent).
- The sample config (`configs/fetch_pipeline.example.json`) references the shipped schema so editors/CI can validate it.

**Interface / behavior implications**
- Operators can rely on `npm run data:fetch -- --config ...` to sanity-check configs today; Future ingestion code must keep the CLI flag semantics and schema keys stable.
- Configs may now set `concurrency.maxOpenSectionsWorkers` (or its downgraded counterpart) to zero to disable polling, matching the doc’s optional-heartbeat description.

**Risks / limits / TODOs**
- Execution engine is still pending (see ST-20251113-act-001-02); current command only plans and logs slices, so downstream teams cannot fetch SOC data yet.
- Schema validation only guards structure; actual schema→runtime parity must be re-validated once the ingest implementation lands.

**Self-test status**
- `npm run data:fetch -- --config configs/fetch_pipeline.example.json --dry-run`

## Code Review - ST-20251113-act-001-01-pipeline-config - 2025-11-17T09:21:40Z
docs/fetch_pipeline.md
Comment on lines +8 to +12


```bash
npm run data:fetch -- \
  --config configs/fetch_pipeline.example.json \
  --mode incremental \
  --terms 12024 \

@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Documented npm entrypoint does not exist

The new CLI section tells operators to run npm run data:fetch, but package.json defines no such script and the referenced runner scripts/fetch_soc_data.ts is absent. Running this command in the current repo immediately fails with “missing script: data:fetch”, so the documented entrypoint is unusable until a matching script is added or the instructions are corrected.

configs/fetch_pipeline.example.json


@@ -0,0 +1,84 @@
{
  "$schema": "./fetch_pipeline.schema.json",


@chatgpt-codex-connector
chatgpt-codex-connector bot
 
P2 Badge Sample config points to missing JSON schema

The example pipeline config sets $schema to ./fetch_pipeline.schema.json, but there is no such schema file anywhere in the repo (find . -name fetch_pipeline.schema.json returns nothing). Any editor or validator that follows the $schema pointer will error or skip validation, defeating the purpose of shipping a schema reference; either include the schema file or point to an existing one.

configs/fetch_pipeline.schema.json
Comment on lines +33 to +37


      "properties": {
        "maxCourseWorkers": { "$ref": "#/$defs/positiveInteger" },
        "courseRequestIntervalMs": { "$ref": "#/$defs/nonNegativeInteger" },
        "maxOpenSectionsWorkers": { "$ref": "#/$defs/positiveInteger" },
        "openSectionsIntervalMs": { "$ref": "#/$defs/nonNegativeInteger" },


@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Allow disabling openSections worker pool

The schema forces concurrency.maxOpenSectionsWorkers to be a positiveInteger (minimum 1), so a config with 0 workers will be rejected. The docs for this pipeline describe the openSections heartbeat as optional and only running when maxOpenSectionsWorkers>0 (see docs/fetch_pipeline.md lines 72–89), meaning operators should be able to disable it by setting the worker count to 0. With the current validation, that documented switch cannot be expressed, so configs that try to turn off the poller will fail schema validation and the CLI cannot be configured as described.

## Code Review - ST-20251113-act-001-01-pipeline-config - 2025-11-17T09:34:56Z
---review-start---
Codex Review: Didn't find any major issues. 🚀
---review-end---
