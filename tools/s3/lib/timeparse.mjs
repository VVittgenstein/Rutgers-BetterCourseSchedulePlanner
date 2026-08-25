// Time parsing: ISO-8601 client timestamps (sub-ms precision, explicit offsets)
// and RFC 7231 HTTP dates (1 s precision server clock).

const ISO_RE =
  /^(\d{4}-\d{2}-\d{2})[Tt ](\d{2}:\d{2}:\d{2})(?:\.(\d+))?(Z|z|[+-]\d{2}:?\d{2})$/;

// Parses an ISO-8601 timestamp to integer epoch milliseconds.
// Fractional seconds beyond milliseconds are truncated toward zero.
// Returns null when unparseable.
export function parseIsoMs(str) {
  if (typeof str !== "string") return null;
  const m = ISO_RE.exec(str.trim());
  if (!m) return null;
  const [, date, time, frac, offset] = m;
  const ms3 = (frac ?? "").padEnd(3, "0").slice(0, 3);
  const normOffset = offset === "z" ? "Z" : offset;
  const parsed = Date.parse(`${date}T${time}.${ms3}${normOffset}`);
  return Number.isFinite(parsed) ? parsed : null;
}

// Parses an RFC 7231 IMF-fixdate (e.g. "Thu, 20 Aug 2026 03:37:00 GMT").
// Returns integer epoch ms of the second start, or null (never fatal).
export function parseHttpDate(str) {
  if (typeof str !== "string" || str.length === 0) return null;
  const parsed = Date.parse(str);
  if (!Number.isFinite(parsed)) return null;
  return parsed;
}
