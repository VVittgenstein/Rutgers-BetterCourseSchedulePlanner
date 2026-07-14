import type { ReactNode } from 'react';

import { ActionButton } from '../../shared/design-system';
import { useBcspI18n } from '../../shared/i18n/runtime';
import { useLocalI18n } from '../i18n/runtime';
import { LocalPersonalStyles } from '../personal/styles';

const LOCAL_PAGE_CSS = String.raw`
.local-page__system-state {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--bcsp-space-3);
  padding: var(--bcsp-space-3);
  border: 1px solid var(--bcsp-line);
  font-family: var(--bcsp-font-data);
  font-size: 0.72rem;
  font-weight: 750;
  letter-spacing: 0.055em;
  line-height: 1.45;
  text-transform: uppercase;
}
.local-page__system-state[role='alert'] { border-left: 0.4rem solid var(--bcsp-accent); }
.local-page__system-state p { margin: 0; }
.local-page__system-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: var(--bcsp-space-2); }
.local-page__inline-panel {
  display: grid;
  gap: var(--bcsp-space-2);
  padding-top: var(--bcsp-space-3);
  border-top: 1px solid var(--bcsp-line);
}
.local-page__inline-panel p { margin: 0; }
.local-page__inline-form { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-2); }
.local-page__inline-actions { display: flex; flex-wrap: wrap; gap: var(--bcsp-space-2); }
.local-page__input {
  width: 100%;
  min-width: 0;
  min-height: 2.75rem;
  padding: 0.65rem 0.75rem;
  border: 1px solid var(--bcsp-line);
  border-radius: 0;
  color: var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
  font: inherit;
}
.local-page__input:focus-visible { outline: 3px solid var(--bcsp-focus, var(--bcsp-accent)); outline-offset: 2px; }
.local-page__technical {
  overflow-wrap: anywhere;
  font-family: var(--bcsp-font-data);
  font-size: 0.72rem;
  letter-spacing: 0.035em;
}
.local-page__success {
  margin: 0;
  padding: var(--bcsp-space-3);
  border: 1px solid var(--bcsp-line);
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
  font-family: var(--bcsp-font-data);
  font-size: 0.7rem;
  font-weight: 750;
  letter-spacing: 0.055em;
  text-transform: uppercase;
}
.local-page__range-row { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: var(--bcsp-space-3); }
.local-page__range-row output {
  min-width: 4rem;
  font-family: var(--bcsp-font-data);
  font-size: 1rem;
  font-weight: 800;
  text-align: right;
}
.local-page__settings-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-3);
  padding: var(--bcsp-space-3);
  border-right: 1px solid var(--bcsp-line);
  border-bottom: 1px solid var(--bcsp-line);
}
.local-page__reset-scope[data-scope='ALL'] { border-top: 0.4rem solid var(--bcsp-accent); }
.local-page__reset-scope h4 { max-width: 20rem; }
.local-page__history-state { display: flex; flex-wrap: wrap; align-items: center; gap: var(--bcsp-space-2); }
@media (max-width: 47.999rem) {
  .local-page__system-state { grid-template-columns: 1fr; }
  .local-page__system-actions { justify-content: flex-start; }
}
@media (max-width: 31.999rem) {
  .local-page__inline-form { grid-template-columns: 1fr; }
  .local-page__inline-actions > .bcsp-action,
  .local-page__settings-actions > .bcsp-action { width: 100%; }
}
`;

export type MaybePromise<T> = T | Promise<T>;

export interface LocalPageAsyncState {
  readonly error?: Error | string | null | undefined;
  readonly onClearError?: (() => void) | undefined;
  readonly onReload?: (() => MaybePromise<void>) | undefined;
  readonly pending?: boolean | undefined;
}

export interface LocalPageFrameProps extends LocalPageAsyncState {
  readonly children: ReactNode;
  readonly intro: string;
  readonly kicker: string;
  readonly title: string;
}

export function LocalPageFrame({
  children,
  error = null,
  onClearError,
  onReload,
  pending = false,
}: LocalPageFrameProps) {
  const local = useLocalI18n();
  const shared = useBcspI18n();
  const errorDetail = error instanceof Error ? error.message : error;

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
            <p>{local.t('local.status.error')}</p>
            {errorDetail.length === 0 ? null : (
              <samp className="local-page__technical">{errorDetail}</samp>
            )}
          </div>
          <div className="local-page__system-actions">
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
