import type { ReactNode } from 'react';

import { ActionButton } from '../../shared/design-system';
import { useBcspI18n } from '../../shared/i18n/runtime';
import { useLocalI18n } from '../i18n/runtime';
import { LocalPersonalStyles } from '../personal/styles';
import { localSyncMessageKey, type LocalSyncStatus } from '../personal/syncFailure';

/*
 * Page-level pieces shared by the three local pages (design spec v2: 4.2,
 * 4.9, 5.6-5.8). Tokens only; no uppercase, no tracking, weights 400/600.
 */
const LOCAL_PAGE_CSS = String.raw`
.local-page__system-state {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--bcsp-space-2);
  padding: var(--bcsp-space-2) var(--bcsp-space-3);
  border: 1px solid var(--bcsp-info-line);
  border-radius: 0.5rem;
  color: var(--bcsp-ink);
  background: var(--bcsp-info-tint);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 400;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-body);
  text-transform: none;
}
.local-page__system-state::before {
  content: '';
  width: 1rem;
  height: 1rem;
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-info);
}
.local-page__system-state[role='alert'] {
  border-color: var(--bcsp-danger-line);
  background: var(--bcsp-danger-tint);
}
.local-page__system-state[role='alert']::before { background: var(--bcsp-danger); }
.local-page__system-state p { margin: 0; }
.local-page__system-state > div { min-width: 0; }
.local-page__system-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: var(--bcsp-space-1); }
.local-page__inline-panel {
  display: grid;
  gap: var(--bcsp-space-2);
  padding: var(--bcsp-space-3);
  border-radius: 0.5rem;
  background: var(--bcsp-paper-raised);
  color: var(--bcsp-ink);
  font-size: var(--bcsp-text-body);
  line-height: var(--bcsp-lh-body);
}
.local-page__inline-panel--danger {
  border: 1px solid var(--bcsp-danger-line);
  background: var(--bcsp-danger-tint);
}
.local-page__inline-panel p { margin: 0; }
.local-page__inline-form { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-2); align-items: end; }
.local-page__inline-actions { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-1); }
.local-page__input {
  width: 100%;
  min-width: 0;
  height: var(--bcsp-control-h);
  min-height: var(--bcsp-control-h);
  padding: 0 var(--bcsp-space-2);
  border: 1px solid var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
  font: inherit;
  font-size: var(--bcsp-text-body);
}
.local-page__technical {
  display: block;
  margin-top: 0.125rem;
  overflow-wrap: anywhere;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-mono);
  font-size: var(--bcsp-text-meta);
  letter-spacing: 0;
  line-height: var(--bcsp-lh-meta);
}
.local-page__success {
  display: flex;
  align-items: center;
  gap: var(--bcsp-space-2);
  margin: 0;
  padding: var(--bcsp-space-2) var(--bcsp-space-3);
  border: 1px solid var(--bcsp-ok-line);
  border-radius: 0.5rem;
  color: var(--bcsp-ink);
  background: var(--bcsp-ok-tint);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 400;
  letter-spacing: 0;
  text-transform: none;
}
.local-page__success::before {
  content: '';
  flex: none;
  width: 1rem;
  height: 1rem;
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-ok);
}
.local-page__range-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: var(--bcsp-space-3); min-height: var(--bcsp-control-h); }
.local-page__range-row output {
  min-width: 3rem;
  font-feature-settings: "tnum" 1;
  font-size: var(--bcsp-text-body);
  font-variant-numeric: tabular-nums;
  font-weight: 600;
  text-align: right;
}
.local-page__settings-actions {
  position: sticky;
  bottom: 0;
  z-index: var(--bcsp-z-sticky-sub);
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-3);
  margin: var(--bcsp-space-3) calc(-1 * var(--bcsp-space-4)) calc(-1 * var(--bcsp-space-4));
  padding: var(--bcsp-space-3) var(--bcsp-space-4);
  border-top: 1px solid var(--bcsp-line-soft);
  border-radius: 0 0 var(--bcsp-radius-3) var(--bcsp-radius-3);
  background: var(--bcsp-paper-raised);
}
.local-page__settings-actions .local-personal__meta { font-size: var(--bcsp-text-data); line-height: var(--bcsp-lh-data); }
.local-page__settings-actions .local-personal__meta[data-state='SAVED'] { color: var(--bcsp-ok); }
.local-page__settings-actions .local-personal__meta[data-state='INVALID'],
.local-page__settings-actions .local-personal__meta[data-state='FAILED'] { color: var(--bcsp-danger); }
.local-page__reset-scope { padding: var(--bcsp-space-4) var(--bcsp-space-3); }
.local-page__reset-scope[data-scope='ALL'] { border: 1px solid var(--bcsp-danger-line); background: var(--bcsp-danger-tint); }
.local-page__reset-scope h4 { max-width: 20rem; }
.local-page__reset-scope > .bcsp-action { width: 100%; margin-top: auto; }
.local-page__reset-scope .local-page__inline-panel { margin-top: auto; }
.local-page__reset-scope[data-scope='ALL'] .local-page__inline-panel { border-color: var(--bcsp-danger-line); background: var(--bcsp-paper-raised); }
.local-page__history-state { display: flex; flex-wrap: wrap; align-items: center; gap: var(--bcsp-space-1); }
@media (hover: hover) and (pointer: fine) {
  .local-page__input:hover:not(:disabled):not(:focus-visible) { border-color: var(--bcsp-ink-muted); }
}
@media (max-width: 47.999rem) {
  .local-page__system-state { grid-template-columns: auto minmax(0, 1fr); }
  .local-page__system-actions { grid-column: 1 / -1; justify-content: flex-start; }
  .local-page__settings-actions { margin: var(--bcsp-space-3) calc(-1 * var(--bcsp-space-3)) calc(-1 * var(--bcsp-space-3)); padding: var(--bcsp-space-2) var(--bcsp-space-3); }
}
@media (max-width: 31.999rem) {
  .local-page__inline-form { grid-template-columns: minmax(0, 1fr); }
  .local-page__inline-actions > .bcsp-action,
  .local-page__settings-actions > .bcsp-action { width: 100%; }
}
`;

export type MaybePromise<T> = T | Promise<T>;

export interface LocalPageAsyncState {
  readonly error?: Error | string | null | undefined;
  /** The provider's sync status; a FAILED notice selects a specific headline. */
  readonly notice?: LocalSyncStatus | undefined;
  readonly onClearError?: (() => void) | undefined;
  readonly onReload?: (() => MaybePromise<void>) | undefined;
  readonly pending?: boolean | undefined;
}

export interface LocalPageFrameProps extends LocalPageAsyncState {
  readonly children: ReactNode;
  readonly intro: string;
  /** Legacy heading eyebrow; the shell heading renders the page title, so nothing reads it. */
  readonly kicker?: string | undefined;
  readonly title: string;
}

export function localFailureMessageKey(notice: LocalSyncStatus | undefined) {
  return notice?.phase === 'FAILED'
    ? localSyncMessageKey(notice) ?? 'local.status.error'
    : 'local.status.error';
}

export function LocalPageFrame({
  children,
  error = null,
  notice,
  onClearError,
  onReload,
  pending = false,
}: LocalPageFrameProps) {
  const local = useLocalI18n();
  const shared = useBcspI18n();
  const errorDetail = error instanceof Error ? error.message : error;
  const offersPageReload = notice?.phase === 'FAILED' && notice.reason === 'STATE_RESET';

  return (
    <div className="local-personal">
      <LocalPersonalStyles />
      <style data-bcsp-local-page="">{LOCAL_PAGE_CSS}</style>
      {pending ? (
        <div aria-live="polite" className="local-page__system-state" role="status">
          <p>{local.t('local.status.busy')}</p>
        </div>
      ) : null}
      {errorDetail === null ? null : (
        <div className="local-page__system-state" role="alert">
          <div>
            <p>{local.t(localFailureMessageKey(notice))}</p>
            {errorDetail.length === 0 ? null : (
              <samp className="local-page__technical">{errorDetail}</samp>
            )}
          </div>
          <div className="local-page__system-actions">
            {offersPageReload ? (
              <ActionButton onClick={() => globalThis.location?.reload()} tone="accent">
                {local.t('local.action.reload_page')}
              </ActionButton>
            ) : null}
            {onReload === undefined ? null : (
              <ActionButton onClick={() => void onReload()} tone="accent">
                {local.t('local.action.reload')}
              </ActionButton>
            )}
            {onClearError === undefined ? null : (
              <ActionButton onClick={onClearError} tone="quiet">
                {shared.t('action.close')}
              </ActionButton>
            )}
          </div>
        </div>
      )}
      {children}
    </div>
  );
}

export async function completeAction(action: () => MaybePromise<void>): Promise<boolean> {
  try {
    await action();
    return true;
  } catch {
    return false;
  }
}
