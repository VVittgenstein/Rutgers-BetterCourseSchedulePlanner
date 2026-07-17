import { SharedApplication } from '../shared/SharedApplication';
import { BcspI18nProvider } from '../shared/i18n/runtime';
import {
  currentBrowserLocaleSource,
  resolvePublicLocale,
  type BrowserLocaleSource,
} from './i18n/localeBootstrap';
import {
  PublicProductBootstrap,
  type PublicProductBootstrapConfiguration,
} from './product/PublicProductBootstrap';

export interface PublicCompositionRootProps {
  readonly localeSource?: BrowserLocaleSource;
  readonly product?: PublicProductBootstrapConfiguration;
}

export function PublicCompositionRoot({ localeSource, product }: PublicCompositionRootProps = {}) {
  const initialLocale = resolvePublicLocale(localeSource ?? currentBrowserLocaleSource());
  return (
    <BcspI18nProvider initialLocale={initialLocale}>
      <PublicProductBootstrap {...product}>
        <SharedApplication />
      </PublicProductBootstrap>
    </BcspI18nProvider>
  );
}
