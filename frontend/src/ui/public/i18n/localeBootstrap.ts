import {
  resolveSupportedLocale,
  type SupportedLocale,
} from '../../shared/i18n/contract';

export interface BrowserLocaleSource {
  readonly language?: string;
  readonly languages?: readonly string[];
}

export function browserLocaleCandidates(source: BrowserLocaleSource): readonly string[] {
  if (source.languages !== undefined && source.languages.length > 0) return source.languages;
  return source.language === undefined ? [] : [source.language];
}

export function resolvePublicLocale(source: BrowserLocaleSource): SupportedLocale {
  return resolveSupportedLocale(browserLocaleCandidates(source));
}

export function currentBrowserLocaleSource(): BrowserLocaleSource {
  if (typeof navigator === 'undefined') return {};
  return { language: navigator.language, languages: navigator.languages };
}
