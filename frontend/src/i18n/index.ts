import i18n, { type Resource } from 'i18next';
import { initReactI18next } from 'react-i18next';

import messages from '../../i18n/messages.json';

export type LocaleKey = keyof typeof messages;

export const supportedLocales = Object.keys(messages) as LocaleKey[];

const resources: Resource = supportedLocales.reduce((acc, locale) => {
  acc[locale] = { translation: messages[locale] };
  return acc;
}, {} as Resource);

const DEFAULT_LOCALE: LocaleKey = 'zh';
const FALLBACK_LOCALE: LocaleKey = 'en';
const LOCALE_STORAGE_KEY = 'bcsp:locale';

const isSupportedLocale = (value: string | null): value is LocaleKey =>
  Boolean(value && supportedLocales.includes(value as LocaleKey));

const resolveLocaleFromValue = (value: string | null): LocaleKey | null => {
  if (isSupportedLocale(value)) return value;
  if (!value) return null;
  const shortened = value.split('-')[0] as string;
  return isSupportedLocale(shortened) ? shortened : null;
};

const readStoredLocale = (): LocaleKey | null => {
  if (typeof window === 'undefined') return null;
  try {
    const value = window.localStorage.getItem(LOCALE_STORAGE_KEY);
    return resolveLocaleFromValue(value);
  } catch {
    return null;
  }
};

const applyDocumentLocale = (locale: LocaleKey) => {
  if (typeof document === 'undefined') return;
  document.documentElement.lang = locale;
};

const initialLocale = readStoredLocale() ?? DEFAULT_LOCALE;
applyDocumentLocale(initialLocale);

i18n.use(initReactI18next).init({
  resources,
  lng: initialLocale,
  fallbackLng: FALLBACK_LOCALE,
  interpolation: {
    escapeValue: false,
  },
  returnNull: false,
  missingKeyHandler(_, namespace, key) {
    // Surface missing keys early while still rendering a fallback.
    console.warn(`[i18n] Missing key "${namespace}:${key}"`);
  },
});

i18n.on('languageChanged', (next) => {
  const resolved = resolveLocaleFromValue(next);
  if (resolved) {
    applyDocumentLocale(resolved);
  }
});

export const localeStorageKey = LOCALE_STORAGE_KEY;

export default i18n;
