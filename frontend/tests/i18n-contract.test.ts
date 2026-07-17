import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import {
  fallbackLocale,
  isMessageKey,
  lookupMessage,
  messages,
  resolveSupportedLocale,
  supportedLocales,
} from '../src/ui/shared/i18n/contract';

interface ContractPair {
  readonly messageKey: string;
  readonly wire: string;
}

interface I18nGolden {
  readonly apiErrors: readonly ContractPair[];
  readonly canonicalLocales: readonly string[];
  readonly fallback: string;
  readonly matchReasons: readonly ContractPair[];
}

interface FilterField {
  readonly i18nKey: string;
}

interface FilterSchema {
  readonly fields: readonly FilterField[];
}

function readJson<T>(relativeUrl: string): T {
  return JSON.parse(readFileSync(new URL(relativeUrl, import.meta.url), 'utf8')) as T;
}

function placeholders(value: string): string[] {
  return [...value.matchAll(/\{\{\s*([A-Za-z0-9_.-]+)[^}]*\}\}/gu)]
    .map((match) => match[1] ?? '')
    .sort();
}

describe('shared i18n contract', () => {
  const golden = readJson<I18nGolden>(
    '../../crates/bcsp-contracts/tests/golden/i18n-contract-v1.json',
  );
  const filterSchema = readJson<FilterSchema>(
    '../../crates/bcsp-contracts/tests/golden/filter-schema-v1.json',
  );

  it('binds the canonical locale and Rust error/reason contracts', () => {
    expect(supportedLocales).toEqual(golden.canonicalLocales);
    expect(fallbackLocale).toBe(golden.fallback);

    const requiredKeys = [
      ...golden.apiErrors.map(({ messageKey }) => messageKey),
      ...golden.matchReasons.map(({ messageKey }) => messageKey),
      ...filterSchema.fields.map(({ i18nKey }) => i18nKey),
    ];
    expect(new Set(requiredKeys).size).toBe(requiredKeys.length);
    for (const key of requiredKeys) {
      expect(isMessageKey(key), key).toBe(true);
      for (const locale of supportedLocales) {
        expect(lookupMessage(locale, key), `${locale}:${key}`).toBeTruthy();
      }
    }
  });

  it('negotiates supported browser language tags in preference order', () => {
    expect(resolveSupportedLocale(['fr-FR', 'zh-Hans-CN'])).toBe('zh-CN');
    expect(resolveSupportedLocale(['en-GB', 'zh-CN'])).toBe('en-US');
    expect(resolveSupportedLocale(['fr-FR'])).toBe('en-US');
    expect(resolveSupportedLocale([])).toBe(fallbackLocale);
  });

  it('keeps both catalogs exactly parallel, nonempty, and placeholder-compatible', () => {
    const englishKeys = Object.keys(messages['en-US']).sort();
    const chineseKeys = Object.keys(messages['zh-CN']).sort();
    expect(chineseKeys).toEqual(englishKeys);

    for (const key of englishKeys) {
      const english = messages['en-US'][key as keyof (typeof messages)['en-US']];
      const chinese = messages['zh-CN'][key as keyof (typeof messages)['zh-CN']];
      expect(english.trim(), `en-US:${key}`).not.toBe('');
      expect(chinese.trim(), `zh-CN:${key}`).not.toBe('');
      expect(`${english}\n${chinese}`, key).not.toMatch(/\b(?:TODO|TBD)\b|\uFFFD/iu);
      expect(placeholders(chinese), key).toEqual(placeholders(english));
    }
  });

  it('contains no local-only or retired notification surfaces', () => {
    expect(Object.keys(messages['en-US']).join('\n')).not.toMatch(
      /saved.?views?|persistent|history|reset|e-?mail|discord|web.?push|calendar|subscription/iu,
    );
  });

  it('never treats raw Rutgers or user-provided values as translation keys', () => {
    const rawRutgersValue = '01:198:112 — Data Structures';
    expect(isMessageKey(rawRutgersValue)).toBe(false);
    expect(lookupMessage('zh-CN', rawRutgersValue)).toBeUndefined();
  });
});
