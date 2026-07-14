import { useBcspI18n } from './i18n/runtime';
import { PRODUCT_PROTOCOL_VERSION, useProductRuntimeState } from './product';

export function SharedApplication() {
  const { locale } = useBcspI18n();
  const productRuntime = useProductRuntimeState();
  return (
    <div
      data-bcsp-locale={locale}
      data-bcsp-product-error={
        productRuntime.status === 'ERROR' ? productRuntime.reason : undefined
      }
      data-bcsp-product-protocol={PRODUCT_PROTOCOL_VERSION}
      data-bcsp-product-state={productRuntime.status}
      data-bcsp-shared-application=""
    />
  );
}
