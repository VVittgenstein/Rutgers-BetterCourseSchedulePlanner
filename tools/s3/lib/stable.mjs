// Deterministic hashing, serialization, and number formatting helpers.

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { AnalyzerError } from "./errors.mjs";

export function sha256File(path) {
  const hash = createHash("sha256");
  hash.update(readFileSync(path));
  return hash.digest("hex");
}

export function sha256Text(str) {
  return createHash("sha256").update(str, "utf8").digest("hex");
}

function serialize(value, indent, out) {
  const pad = "  ".repeat(indent);
  const padInner = "  ".repeat(indent + 1);
  if (value === null) {
    out.push("null");
    return;
  }
  const t = typeof value;
  if (t === "number") {
    if (!Number.isFinite(value)) {
      throw new AnalyzerError("E_INTERNAL", "non-finite number in output");
    }
    out.push(JSON.stringify(value));
    return;
  }
  if (t === "string" || t === "boolean") {
    out.push(JSON.stringify(value));
    return;
  }
  if (t === "undefined") {
    throw new AnalyzerError("E_INTERNAL", "undefined value in output");
  }
  if (Array.isArray(value)) {
    if (value.length === 0) {
      out.push("[]");
      return;
    }
    out.push("[\n");
    value.forEach((item, i) => {
      out.push(padInner);
      serialize(item, indent + 1, out);
      out.push(i < value.length - 1 ? ",\n" : "\n");
    });
    out.push(pad + "]");
    return;
  }
  if (t === "object") {
    const keys = Object.keys(value); // insertion order by construction
    if (keys.length === 0) {
      out.push("{}");
      return;
    }
    out.push("{\n");
    keys.forEach((key, i) => {
      if (value[key] === undefined) {
        throw new AnalyzerError("E_INTERNAL", `undefined value for key ${key}`);
      }
      out.push(padInner + JSON.stringify(key) + ": ");
      serialize(value[key], indent + 1, out);
      out.push(i < keys.length - 1 ? ",\n" : "\n");
    });
    out.push(pad + "}");
    return;
  }
  throw new AnalyzerError("E_INTERNAL", `unserializable value of type ${t}`);
}

// Objects serialize keys in insertion order; arrays in order; 2-space indent; LF only.
export function stableStringify(value) {
  const out = [];
  serialize(value, 0, out);
  return out.join("");
}

// The only permitted non-integer numbers in outputs come from fmtRatio / fmtMs1.
export function fmtRatio(x) {
  return Number(x.toFixed(4));
}

export function fmtMs1(x) {
  return Number(x.toFixed(1));
}

export function isoUtcMs(epochMs) {
  return new Date(epochMs).toISOString();
}
