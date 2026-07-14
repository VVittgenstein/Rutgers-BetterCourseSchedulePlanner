import { useBcspI18n } from './i18n/runtime';

export function SharedApplication() {
  const { locale } = useBcspI18n();
  return <div data-bcsp-locale={locale} data-bcsp-shared-application="" />;
}
