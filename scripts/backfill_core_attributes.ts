#!/usr/bin/env tsx
import Database from 'better-sqlite3';
import fs from 'node:fs';
import path from 'node:path';

type Attr = {
  code: string;
  referenceId: string | null;
  effectiveTerm: string | null;
  metadata: string | null;
};

function main() {
  const sqliteFile = process.env.SQLITE_FILE ?? path.resolve('data', 'courses.sqlite');
  if (!fs.existsSync(sqliteFile)) {
    throw new Error(`SQLite file not found: ${sqliteFile}`);
  }

  const db = new Database(sqliteFile);
  db.pragma('foreign_keys = ON');

  const selectCourses = db.prepare<never, { course_id: number; term_id: string; core_json: string | null }>(
    'SELECT course_id, term_id, core_json FROM courses',
  );
  const deleteAll = db.prepare('DELETE FROM course_core_attributes');
  const insertAttr = db.prepare(
    'INSERT INTO course_core_attributes (course_id, term_id, core_code, reference_id, effective_term, metadata) VALUES (?, ?, ?, ?, ?, ?)',
  );
  const updateHasCore = db.prepare('UPDATE courses SET has_core_attribute = ? WHERE course_id = ?');

  const stats = { courses: 0, attributes: 0 };

  const tx = db.transaction(() => {
    deleteAll.run();
    for (const row of selectCourses.all()) {
      stats.courses += 1;
      const attrs = parseCoreAttributes(row.core_json);
      updateHasCore.run(attrs.length > 0 ? 1 : 0, row.course_id);
      for (const attr of attrs) {
        insertAttr.run(row.course_id, row.term_id, attr.code, attr.referenceId, attr.effectiveTerm, attr.metadata);
        stats.attributes += 1;
      }
    }
  });

  tx();
  console.log(
    `Backfill complete. Courses processed=${stats.courses}, core attributes inserted=${stats.attributes}, sqlite=${sqliteFile}`,
  );
}

function parseCoreAttributes(raw: string | null): Attr[] {
  if (!raw || raw.trim().length === 0) return [];
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) return [];

  const seen = new Set<string>();
  const result: Attr[] = [];
  for (const entry of parsed) {
    const normalized = normalizeAttr(entry);
    if (!normalized) continue;
    if (seen.has(normalized.code)) continue;
    seen.add(normalized.code);
    result.push(normalized);
  }
  return result;
}

function normalizeAttr(entry: unknown): Attr | null {
  if (entry === null || entry === undefined) return null;
  if (typeof entry === 'string' || typeof entry === 'number') {
    const code = normalizeCode(entry);
    if (!code) return null;
    return {
      code,
      referenceId: null,
      effectiveTerm: null,
      metadata: safeStringify({ code, source: 'string' }),
    };
  }

  if (typeof entry !== 'object') return null;
  const obj = entry as Record<string, unknown>;
  const code = normalizeCode(obj.coreCode ?? obj.code ?? obj.core_code);
  if (!code) return null;

  const referenceId = normalizeNullable(obj.coreCodeReferenceId ?? obj.referenceId);
  const effectiveTerm = normalizeNullable(obj.effective ?? obj.effectiveTerm ?? obj.term);
  const description =
    normalizeNullable(obj.description) ??
    normalizeNullable((obj as Record<string, unknown>).coreCodeDescription) ??
    normalizeNullable((obj as Record<string, unknown>).coreDescription) ??
    null;

  return {
    code,
    referenceId,
    effectiveTerm,
    metadata: safeStringify({ ...obj, code, referenceId, effectiveTerm, description }),
  };
}

function normalizeCode(value: unknown): string | null {
  if (typeof value === 'string' || typeof value === 'number') {
    const normalized = String(value).trim().toUpperCase();
    return normalized.length > 0 ? normalized : null;
  }
  return null;
}

function normalizeNullable(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
    const normalized = String(value).trim();
    return normalized.length > 0 ? normalized : null;
  }
  return null;
}

function safeStringify(value: unknown): string | null {
  try {
    return JSON.stringify(value);
  } catch {
    return null;
  }
}

try {
  main();
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
