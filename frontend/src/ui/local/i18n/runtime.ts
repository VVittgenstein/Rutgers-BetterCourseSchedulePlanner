import { useMemo } from 'react';

import { useBcspI18n } from '../../shared/i18n/runtime';
import {
  localMessage,
  type LocalInterpolation,
  type LocalMessageKey,
} from './catalog';

export function useLocalI18n() {
  const shared = useBcspI18n();
  return useMemo(() => ({
    formatDate: shared.formatDate,
    formatNumber: shared.formatNumber,
    locale: shared.locale,
    t: (key: LocalMessageKey, values?: LocalInterpolation) =>
      localMessage(shared.locale, key, values),
  }), [shared]);
}
