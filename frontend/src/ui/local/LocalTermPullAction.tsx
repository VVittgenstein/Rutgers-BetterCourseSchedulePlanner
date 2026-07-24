import { useEffect, useId, useState } from 'react';

import { useBcspI18n } from '../shared/i18n/runtime';
import {
  resolveQueryScopeAction,
  type QueryScopeUnavailableActionContext,
} from '../shared/search/QueryScopeControl';
import { useLocalI18n } from './i18n/runtime';

export interface LocalTermPullActionProps extends QueryScopeUnavailableActionContext {
  readonly onPull: (term: string) => Promise<void>;
}

export function LocalTermPullAction({
  input,
  onPull,
}: LocalTermPullActionProps) {
  const shared = useBcspI18n();
  const local = useLocalI18n();
  const disabledReasonId = useId();
  const publicationReasonId = useId();
  const failureId = useId();
  const [pullingTerm, setPullingTerm] = useState<string | null>(null);
  const [requestedPull, setRequestedPull] = useState<{
    readonly terminalEvidence: string;
    readonly term: string;
  } | null>(null);
  const [failedTerm, setFailedTerm] = useState<string | null>(null);
  const term = input.term;
  const requestPending = pullingTerm === term?.term || requestedPull?.term === term?.term;
  const action = resolveQueryScopeAction({
    ...input,
    pullAllowed: term?.manualPullAllowed === true,
    pullRequestPending: requestPending,
  });
  const { active, allReady, terminalEvidence: currentTerminalEvidence } = action.observation;

  useEffect(() => {
    if (requestedPull === null || requestedPull.term !== term?.term) return;
    if (
      active
      || allReady
      || requestedPull.terminalEvidence !== currentTerminalEvidence
    ) {
      setRequestedPull(null);
    }
  }, [active, allReady, currentTerminalEvidence, requestedPull, term?.term]);

  const runPull = () => {
    if (!action.enabled || action.kind !== 'PULL' || term === null) return;
    const selectedTerm = term.term;
    setFailedTerm(null);
    setRequestedPull({ term: selectedTerm, terminalEvidence: currentTerminalEvidence });
    setPullingTerm(selectedTerm);
    void onPull(selectedTerm).catch(() => {
      setRequestedPull(null);
      setFailedTerm(selectedTerm);
    }).finally(() => setPullingTerm(null));
  };

  const reason = action.reason === 'VALIDATING'
    ? shared.t('scope.validating')
    : action.reason === 'PULL_ACTIVE'
      ? local.t('local.scope.pull_active')
      : action.reason === 'TERM_WINDOW_UNAVAILABLE'
        ? shared.t('scope.term_window_unavailable')
        : action.reason === 'SELECT_READY_CAMPUS'
          ? shared.t('scope.select_ready_campus')
          : action.reason === 'SELECTED_TARGET_NOT_READY'
            ? shared.t('scope.selected_not_ready')
            : null;
  const accessiblePublicationReason = action.reason === 'PULL_UNPUBLISHED'
    ? shared.t('scope.term_state_unpublished')
    : action.reason === 'PULL_PUBLICATION_UNKNOWN'
      ? shared.t('scope.term_state_publication_unknown')
      : null;

  return (
    <>
      <button
        aria-busy={requestPending || undefined}
        aria-describedby={[
          accessiblePublicationReason === null ? null : publicationReasonId,
          reason === null ? null : disabledReasonId,
          failedTerm === term?.term ? failureId : null,
        ].filter((value): value is string => value !== null).join(' ') || undefined}
        className="bcsp-action bcsp-action--accent"
        disabled={!action.enabled}
        onClick={runPull}
        type="button"
      >
        {action.kind === 'PULL'
          ? local.t('local.scope.pull')
          : shared.t(action.kind === 'APPLIED' ? 'action.applied' : 'action.apply')}
      </button>
      {accessiblePublicationReason === null ? null : (
        <span hidden id={publicationReasonId}>
          {accessiblePublicationReason}
        </span>
      )}
      {reason === null ? null : <p className="query-scope__status" id={disabledReasonId}>{reason}</p>}
      {failedTerm === term?.term ? (
        <p className="query-scope__error" id={failureId} role="alert">{local.t('local.scope.pull_failed')}</p>
      ) : null}
    </>
  );
}
