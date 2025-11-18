import i18n, { type Resource } from 'i18next';
import { initReactI18next } from 'react-i18next';

import messages from '../../i18n/messages.json';

type LocaleKey = keyof typeof messages;

const resources: Resource = (Object.keys(messages) as LocaleKey[]).reduce((acc, locale) => {
  acc[locale] = { translation: messages[locale] };
  return acc;
}, {} as Resource);

const DEFAULT_LOCALE: LocaleKey = 'zh';
const FALLBACK_LOCALE: LocaleKey = 'en';

i18n.use(initReactI18next).init({
  resources,
  lng: DEFAULT_LOCALE,
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

export const supportedLocales = Object.keys(messages) as LocaleKey[];

export default i18n;
