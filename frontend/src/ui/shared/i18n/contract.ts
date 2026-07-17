import { enUSCatalog, zhCNCatalog, type CatalogMessageKey } from './catalog';

export const supportedLocales = ['en-US', 'zh-CN'] as const;
export type SupportedLocale = (typeof supportedLocales)[number];
export const fallbackLocale: SupportedLocale = 'en-US';
export type MessageKey = CatalogMessageKey;

export const messages: Readonly<
  Record<SupportedLocale, Readonly<Record<MessageKey, string>>>
> = {
  'en-US': enUSCatalog,
  'zh-CN': zhCNCatalog,
};

const messageKeys = new Set<string>(Object.keys(enUSCatalog));

export function isSupportedLocale(value: unknown): value is SupportedLocale {
  return typeof value === 'string' && supportedLocales.some((locale) => locale === value);
}

export function resolveSupportedLocale(languages: readonly string[]): SupportedLocale {
  for (const language of languages) {
    const normalized = language.trim().replaceAll('_', '-').toLocaleLowerCase('en-US');
    if (normalized === 'zh' || normalized.startsWith('zh-')) return 'zh-CN';
    if (normalized === 'en' || normalized.startsWith('en-')) return 'en-US';
  }
  return fallbackLocale;
}

export function isMessageKey(value: string): value is MessageKey {
  return messageKeys.has(value);
}

export function lookupMessage(locale: SupportedLocale, key: MessageKey): string;
export function lookupMessage(locale: SupportedLocale, key: string): string | undefined;
export function lookupMessage(locale: SupportedLocale, key: string): string | undefined {
  if (!isMessageKey(key)) return undefined;
  return messages[locale][key];
}
