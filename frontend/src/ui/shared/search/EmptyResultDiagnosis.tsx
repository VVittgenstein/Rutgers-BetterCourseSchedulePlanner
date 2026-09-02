import { useId } from 'react';

import { useBcspI18n } from '../i18n/runtime';
import type { FilterFieldId } from '../product';
import type { DiagnosisState, Relaxation } from './useEmptyResultDiagnosis';

export interface EmptyResultDiagnosisProps {
  readonly disabled: boolean;
  readonly labelFor: (stableId: FilterFieldId) => string;
  readonly onRelax: (relaxation: Relaxation) => void;
  readonly state: DiagnosisState;
}

export function EmptyResultDiagnosis({
  disabled,
  labelFor,
  onRelax,
  state,
}: EmptyResultDiagnosisProps) {
  const i18n = useBcspI18n();
  const titleId = useId();
  if (state.status === 'IDLE') return null;
  // Nothing was active besides the scope: there is no single condition to blame.
  if (state.status === 'READY' && state.rows.length === 0) return null;
  const positiveRows = state.status === 'READY' ? state.rows.filter(({ total }) => total > 0) : [];
  let body;
  if (state.status === 'PROBING') {
    body = <p>{i18n.t('search.diagnosis.probing')}</p>;
  } else if (state.status === 'UNAVAILABLE') {
    body = <p>{i18n.t('search.diagnosis.unavailable')}</p>;
  } else if (positiveRows.length === 0) {
    body = <p>{i18n.t('search.diagnosis.no_single_fix')}</p>;
  } else {
    body = (
      <>
        <p>{i18n.t('search.diagnosis.intro')}</p>
        <ul className="bcsp-search-diagnosis__list">
          {positiveRows.map(({ relaxation, total }) => {
            const label = labelFor(relaxation.stableId);
            const rowLabel = relaxation.kind === 'CLEAR_FIELD'
              ? i18n.t('search.diagnosis.remove_filter', { label })
              : i18n.t('search.diagnosis.include_incomplete', { label });
            return (
              <li key={`${relaxation.kind}:${relaxation.stableId}`}>
                <button
                  className="bcsp-search-diagnosis__action"
                  data-relaxation={`${relaxation.kind}:${relaxation.stableId}`}
                  disabled={disabled}
                  onClick={() => onRelax(relaxation)}
                  type="button"
                >
                  <span>{rowLabel}</span>
                  {' '}
                  <data value={total}>
                    {i18n.t('search.diagnosis.count', { count: i18n.formatNumber(total) })}
                  </data>
                </button>
              </li>
            );
          })}
        </ul>
      </>
    );
  }
  return (
    <section
      aria-busy={state.status === 'PROBING' || undefined}
      aria-labelledby={titleId}
      aria-live="polite"
      className="bcsp-search-diagnosis"
      data-diagnosis-state={state.status}
      role="status"
    >
      <h5 id={titleId}>{i18n.t('search.diagnosis.title')}</h5>
      {body}
    </section>
  );
}
