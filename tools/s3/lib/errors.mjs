// Error taxonomy for the S3 offline rebuild-cadence analyzer.
// Fail-closed: any exit-2 code means no output files are written.

export const EXIT_OK = 0;
export const EXIT_USAGE = 1;
export const EXIT_FAIL_CLOSED = 2;

const EXIT_BY_CODE = new Map([
  ["E_USAGE", EXIT_USAGE],
  ["E_INPUT_MISSING", EXIT_FAIL_CLOSED],
  ["E_NDJSON_PARSE", EXIT_FAIL_CLOSED],
  ["E_SCHEMA_VERSION", EXIT_FAIL_CLOSED],
  ["E_MISSING_FIELD", EXIT_FAIL_CLOSED],
  ["E_SEQUENCE_ORDER", EXIT_FAIL_CLOSED],
  ["E_TIME_PARSE", EXIT_FAIL_CLOSED],
  ["E_TIME_REGRESSION", EXIT_FAIL_CLOSED],
  ["E_SQLITE_SCHEMA", EXIT_FAIL_CLOSED],
  ["E_INPUT_MUTATED", EXIT_FAIL_CLOSED],
  ["E_INTERNAL", EXIT_FAIL_CLOSED],
]);

export class AnalyzerError extends Error {
  constructor(code, message) {
    super(message);
    this.name = "AnalyzerError";
    this.code = code;
    if (!EXIT_BY_CODE.has(code)) {
      // Unknown code is itself an internal defect; keep fail-closed semantics.
      this.code = "E_INTERNAL";
    }
  }

  get exitCode() {
    return EXIT_BY_CODE.get(this.code) ?? EXIT_FAIL_CLOSED;
  }
}

export function internalAssert(condition, message) {
  if (!condition) {
    throw new AnalyzerError("E_INTERNAL", message);
  }
}
