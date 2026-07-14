import { SharedApplication } from '../shared/SharedApplication';
import { BcspI18nProvider } from '../shared/i18n/runtime';
import {
  currentBrowserLocaleSource,
  resolvePublicLocale,
  type BrowserLocaleSource,
} from './i18n/localeBootstrap';

export interface PublicCompositionRootProps {
  readonly localeSource?: BrowserLocaleSource;
}

export function PublicCompositionRoot({ localeSource }: PublicCompositionRootProps = {}) {
  const initialLocale = resolvePublicLocale(localeSource ?? currentBrowserLocaleSource());
  return (
    <BcspI18nProvider initialLocale={initialLocale}>
      <SharedApplication />
    </BcspI18nProvider>
  );
}
