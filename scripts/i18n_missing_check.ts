import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

type LocaleTree = Record<string, unknown>;
type FlatMap = Map<string, string>;

const flattenEntries = (input: unknown, prefix: string, acc: FlatMap): FlatMap => {
  if (Array.isArray(input)) {
    input.forEach((entry, index) => {
      const nextKey = prefix ? `${prefix}.${index}` : `${index}`;
      flattenEntries(entry, nextKey, acc);
    });
    return acc;
  }

  if (input && typeof input === 'object') {
    Object.entries(input as LocaleTree).forEach(([key, value]) => {
      const nextKey = prefix ? `${prefix}.${key}` : key;
      flattenEntries(value, nextKey, acc);
    });
    return acc;
  }

  const typeLabel =
    input === null ? 'null' : typeof input === 'undefined' ? 'undefined' : typeof input;
  acc.set(prefix, typeLabel);
  return acc;
};

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const messagesPath = path.resolve(__dirname, '../frontend/i18n/messages.json');

const raw = await readFile(messagesPath, 'utf-8');
const messages: Record<string, LocaleTree> = JSON.parse(raw);
const locales = Object.keys(messages);

if (locales.length === 0) {
  console.error('[i18n] No locales found in messages.json');
  process.exit(1);
}

const referenceLocale = locales.includes('en') ? 'en' : locales[0];
const referenceTree = messages[referenceLocale];
const referenceMap = flattenEntries(referenceTree, '', new Map());

const problems: string[] = [];

for (const locale of locales) {
  const localeMap = flattenEntries(messages[locale], '', new Map());
  const missingKeys = [...referenceMap.keys()].filter((key) => !localeMap.has(key));
  const typeMismatches = [...referenceMap.keys()].filter((key) => {
    if (!localeMap.has(key)) return false;
    return localeMap.get(key) !== referenceMap.get(key);
  });

  if (missingKeys.length > 0) {
    problems.push(
      `[${locale}] missing ${missingKeys.length} key(s):\n  - ${missingKeys.sort().join('\n  - ')}`,
    );
  }

  if (typeMismatches.length > 0) {
    problems.push(
      `[${locale}] type mismatches on ${typeMismatches.length} key(s):\n  - ${typeMismatches
        .sort()
        .join('\n  - ')}`,
    );
  }
}

if (problems.length > 0) {
  console.error('[i18n] Translation check failed:\n');
  console.error(problems.join('\n\n'));
  process.exit(1);
}

console.log(
  `[i18n] ${locales.length} locale(s) checked successfully using "${referenceLocale}" as reference.`,
);
