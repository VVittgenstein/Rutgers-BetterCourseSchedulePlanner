import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import i18n, { localeStorageKey, supportedLocales, type LocaleKey } from '../i18n';
import { classNames } from '../utils/classNames';
import './LanguageSwitcher.css';

const resolveLocale = (value: string | null): LocaleKey => {
  const exactMatch = supportedLocales.find((locale) => locale === value);
  if (exactMatch) return exactMatch;

  const shortened = value?.split('-')[0];
  const shortMatch = supportedLocales.find((locale) => locale === shortened);
  return shortMatch ?? supportedLocales[0];
};

const persistLocalePreference = (locale: LocaleKey) => {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(localeStorageKey, locale);
  } catch {
    // Ignore storage errors (e.g., private mode).
  }
};

export function LanguageSwitcher() {
  const { t } = useTranslation();
  const [activeLocale, setActiveLocale] = useState<LocaleKey>(() => resolveLocale(i18n.language ?? null));

  useEffect(() => {
    const handler = (nextLocale: string) => {
      setActiveLocale(resolveLocale(nextLocale));
    };
    i18n.on('languageChanged', handler);
    return () => {
      i18n.off('languageChanged', handler);
    };
  }, []);

  const options = useMemo(
    () =>
      supportedLocales.map((locale) => ({
        value: locale,
        label: t(`common.languageSwitcher.languages.${locale}`),
      })),
    [t],
  );

  if (options.length < 2) {
    return null;
  }

  const handleSelect = (locale: LocaleKey) => {
    if (locale === activeLocale) return;
    setActiveLocale(locale);
    persistLocalePreference(locale);
    void i18n.changeLanguage(locale);
  };

  return (
    <div className="language-switcher" role="group" aria-label={t('common.languageSwitcher.label')}>
      <span className="language-switcher__label">{t('common.languageSwitcher.label')}</span>
      <div className="language-switcher__options">
        {options.map((option) => (
          <button
            key={option.value}
            type="button"
            className={classNames(
              'language-switcher__option',
              option.value === activeLocale && 'language-switcher__option--active',
            )}
            onClick={() => handleSelect(option.value)}
            aria-pressed={option.value === activeLocale}
          >
            {option.label}
          </button>
        ))}
      </div>
    </div>
  );
}
