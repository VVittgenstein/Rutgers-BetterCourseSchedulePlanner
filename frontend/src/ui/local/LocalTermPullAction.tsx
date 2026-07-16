import { useState } from 'react';

import { useBcspI18n } from '../shared/i18n/runtime';
import { isServiceStatusV2 } from '../shared/product';
import type { QueryScopeUnavailableActionContext } from '../shared/search/QueryScopeControl';
import { useLocalI18n } from './i18n/runtime';

export interface LocalTermPullActionProps extends QueryScopeUnavailableActionContext {
  readonly onPull: (term: string) => Promise<void>;
}

export function LocalTermPullAction({
  actionPending,
  onPull,
  status,
  term,
}: LocalTermPullActionProps) {
  const shared = useBcspI18n();
  const local = useLocalI18n();
  const [pullingTerm, setPullingTerm] = useState<string | null>(null);
  const [requestedTerms, setRequestedTerms] = useState<ReadonlySet<string>>(() => new Set());
  const [failedTerm, setFailedTerm] = useState<string | null>(null);
  const manual = term?.manualPullAllowed === true;
  const termTargets = status !== null && isServiceStatusV2(status) && term !== null
    ? status.targets.filter(({ target }) => target.term === term.term)
    : [];
  const pullStarted = term !== null && (
    requestedTerms.has(term.term)
    || pullingTerm === term.term
    || termTargets.some((target) =>
      target.snapshotAvailability !== 'UNREQUESTED' || target.workState !== 'IDLE')
  );
  const disabled = actionPending || !manual || term?.discovered !== true || pullStarted;

  const runPull = () => {
    if (disabled || term === null) return;
    const selectedTerm = term.term;
    setFailedTerm(null);
    setPullingTerm(selectedTerm);
    void onPull(selectedTerm).then(() => {
      setRequestedTerms((previous) => new Set(previous).add(selectedTerm));
    }).catch(() => setFailedTerm(selectedTerm)).finally(() => setPullingTerm(null));
  };

  return (
    <>
      <button
        className="bcsp-action bcsp-action--accent"
        disabled={disabled}
        onClick={runPull}
        type="button"
      >
        {manual ? local.t('local.scope.pull') : shared.t('action.apply')}
      </button>
      {failedTerm === term?.term ? (
        <p className="query-scope__error" role="alert">{local.t('local.scope.pull_failed')}</p>
      ) : null}
    </>
  );
}
