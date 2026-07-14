import { SharedApplication } from '../shared/SharedApplication';
import type { SupportedLocale } from '../shared/i18n/contract';
import { BcspI18nProvider } from '../shared/i18n/runtime';
import {
  currentSystemLanguages,
  resolveLocalLocale,
} from './i18n/localeBootstrap';
import {
  LocalProductBootstrap,
  type LocalProductBootstrapConfiguration,
} from './product/LocalProductBootstrap';

export interface LocalCompositionRootProps {
  readonly onLocaleChange?: ((locale: SupportedLocale) => void) | undefined;
  readonly product?: LocalProductBootstrapConfiguration;
  readonly savedLocale?: string | null | undefined;
  readonly systemLanguages?: readonly string[];
}

export function LocalCompositionRoot({
  onLocaleChange,
  product,
  savedLocale,
  systemLanguages,
}: LocalCompositionRootProps = {}) {
  const initialLocale = resolveLocalLocale({
    savedLocale,
    systemLanguages: systemLanguages ?? currentSystemLanguages(),
  });
  return (
    <BcspI18nProvider initialLocale={initialLocale} onLocaleChange={onLocaleChange}>
      <LocalProductBootstrap {...product}>
        <SharedApplication />
      </LocalProductBootstrap>
    </BcspI18nProvider>
  );
}
