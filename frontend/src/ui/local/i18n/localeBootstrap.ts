import {
  isSupportedLocale,
  resolveSupportedLocale,
  type SupportedLocale,
} from '../../shared/i18n/contract';

export interface LocalLocaleBootstrapInput {
  readonly savedLocale?: string | null | undefined;
  readonly systemLanguages: readonly string[];
}

export function resolveLocalLocale({
  savedLocale,
  systemLanguages,
}: LocalLocaleBootstrapInput): SupportedLocale {
  return isSupportedLocale(savedLocale) ? savedLocale : resolveSupportedLocale(systemLanguages);
}

export function resolveLocalLocaleAfterReset(
  systemLanguages: readonly string[],
): SupportedLocale {
  return resolveSupportedLocale(systemLanguages);
}

export function currentSystemLanguages(): readonly string[] {
  if (typeof navigator === 'undefined') return [];
  return navigator.languages.length > 0 ? navigator.languages : [navigator.language];
}
