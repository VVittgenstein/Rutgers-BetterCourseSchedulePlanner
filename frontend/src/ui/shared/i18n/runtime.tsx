import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { createInstance, type i18n } from 'i18next';
import { I18nextProvider, initReactI18next } from 'react-i18next';

import {
  fallbackLocale,
  isSupportedLocale,
  lookupMessage,
  messages,
  resolveSupportedLocale,
  type MessageKey,
  type SupportedLocale,
} from './contract';

export interface BcspI18nProviderProps {
  readonly children: ReactNode;
  readonly initialLocale: SupportedLocale;
  readonly onLocaleChange?: ((locale: SupportedLocale) => void) | undefined;
}

export interface BcspI18nRuntime {
  readonly locale: SupportedLocale;
  readonly changeLocale: (locale: SupportedLocale) => Promise<void>;
  readonly t: (
    key: MessageKey,
    values?: Readonly<Record<string, string | number>>,
  ) => string;
  readonly formatNumber: (
    value: number | bigint,
    options?: Intl.NumberFormatOptions,
  ) => string;
  readonly formatDate: (
    value: Date | number,
    options?: Intl.DateTimeFormatOptions,
  ) => string;
}

const BcspI18nContext = createContext<BcspI18nRuntime | null>(null);

function createI18n(initialLocale: SupportedLocale): i18n {
  const instance = createInstance();
  void instance.use(initReactI18next).init({
    fallbackLng: fallbackLocale,
    initAsync: false,
    interpolation: { escapeValue: false },
    keySeparator: false,
    lng: initialLocale,
    resources: Object.fromEntries(
      Object.entries(messages).map(([locale, catalog]) => [
        locale,
        { translation: catalog },
      ]),
    ),
    returnNull: false,
  });
  return instance;
}

function normalizeLocale(value: string | undefined): SupportedLocale {
  return isSupportedLocale(value) ? value : resolveSupportedLocale([value ?? fallbackLocale]);
}

export function BcspI18nProvider({
  children,
  initialLocale,
  onLocaleChange,
}: BcspI18nProviderProps) {
  const [instance] = useState(() => createI18n(initialLocale));
  const [locale, setLocale] = useState<SupportedLocale>(initialLocale);

  useEffect(() => {
    if (instance.language !== initialLocale) {
      void instance.changeLanguage(initialLocale);
      setLocale(initialLocale);
    }
  }, [initialLocale, instance]);

  useEffect(() => {
    const handleLanguageChanged = (nextLocale: string) => {
      setLocale(normalizeLocale(nextLocale));
    };
    instance.on('languageChanged', handleLanguageChanged);
    return () => {
      instance.off('languageChanged', handleLanguageChanged);
    };
  }, [instance]);

  useEffect(() => {
    const root = document.documentElement;
    const previousLocale = root.lang;
    root.lang = locale;
    return () => {
      if (root.lang === locale) root.lang = previousLocale;
    };
  }, [locale]);

  const changeLocale = useCallback(
    async (nextLocale: SupportedLocale) => {
      if (!isSupportedLocale(nextLocale)) return;
      await instance.changeLanguage(nextLocale);
      onLocaleChange?.(nextLocale);
    },
    [instance, onLocaleChange],
  );

  const value = useMemo<BcspI18nRuntime>(
    () => ({
      changeLocale,
      formatDate: (date, options) => new Intl.DateTimeFormat(locale, options).format(date),
      formatNumber: (number, options) => new Intl.NumberFormat(locale, options).format(number),
      locale,
      t: (key, values) => instance.t(key, {
        defaultValue: lookupMessage(locale, key),
        ...values,
      }),
    }),
    [changeLocale, instance, locale],
  );

  return (
    <I18nextProvider i18n={instance}>
      <BcspI18nContext.Provider value={value}>{children}</BcspI18nContext.Provider>
    </I18nextProvider>
  );
}

export function useBcspI18n(): BcspI18nRuntime {
  const runtime = useContext(BcspI18nContext);
  if (runtime === null) throw new Error('useBcspI18n must be used within BcspI18nProvider');
  return runtime;
}
