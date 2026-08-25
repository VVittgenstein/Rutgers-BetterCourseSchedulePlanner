// CLI argument parsing and normalized-command construction.

import { AnalyzerError } from "./errors.mjs";

export const USAGE = `Usage:
  node tools/s3/rebuild-profile-analyzer.mjs
    --ndjson <path>          # repeatable; samples.ndjson file or run directory
    --sqlite <path>          # repeatable; SQLite db with open_batch_observations
    --sqlite-target <id>     # optional, repeatable; filter SQLite target_id values
    --out-json <path>        # required; machine JSON output
    --out-md <path>          # required; Markdown report output
    --help
`;

export function parseArgs(argv) {
  const result = {
    ndjsonPaths: [],
    sqlitePaths: [],
    sqliteTargets: [],
    outJson: null,
    outMd: null,
    help: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i];
    if (flag === "--help") {
      result.help = true;
      continue;
    }
    const takesValue = [
      "--ndjson",
      "--sqlite",
      "--sqlite-target",
      "--out-json",
      "--out-md",
    ].includes(flag);
    if (!takesValue) {
      throw new AnalyzerError("E_USAGE", `unknown flag: ${flag}`);
    }
    const value = argv[i + 1];
    if (value === undefined || value.startsWith("--")) {
      throw new AnalyzerError("E_USAGE", `missing value for ${flag}`);
    }
    i += 1;
    switch (flag) {
      case "--ndjson":
        result.ndjsonPaths.push(value);
        break;
      case "--sqlite":
        result.sqlitePaths.push(value);
        break;
      case "--sqlite-target":
        result.sqliteTargets.push(value);
        break;
      case "--out-json":
        result.outJson = value;
        break;
      case "--out-md":
        result.outMd = value;
        break;
      default:
        throw new AnalyzerError("E_USAGE", `unknown flag: ${flag}`);
    }
  }
  if (result.help) return result;
  if (result.ndjsonPaths.length === 0 && result.sqlitePaths.length === 0) {
    throw new AnalyzerError("E_USAGE", "no inputs: pass --ndjson and/or --sqlite");
  }
  if (!result.outJson) {
    throw new AnalyzerError("E_USAGE", "--out-json is required");
  }
  if (!result.outMd) {
    throw new AnalyzerError("E_USAGE", "--out-md is required");
  }
  return result;
}

// Built AFTER ingestion from input descriptors, never from raw argv, so the
// rendered command carries basenames only and is independent of argv order.
export function buildNormalizedCommand({ inputs, sqliteTargets, outJsonBase, outMdBase }) {
  const ndjsonParts = inputs
    .filter((inp) => inp.kind === "ndjson")
    .map((inp) => ` --ndjson ${inp.id}`)
    .sort();
  const sqliteParts = inputs
    .filter((inp) => inp.kind === "sqlite")
    .map((inp) => ` --sqlite ${inp.id}`)
    .sort();
  const targetParts = [...sqliteTargets].map((t) => ` --sqlite-target ${t}`).sort();
  return (
    "rebuild-profile-analyzer" +
    ndjsonParts.join("") +
    sqliteParts.join("") +
    targetParts.join("") +
    ` --out-json ${outJsonBase} --out-md ${outMdBase}`
  );
}
