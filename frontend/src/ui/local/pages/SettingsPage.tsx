import { useEffect, useId, useState, type FormEvent } from 'react';

import { ActionButton } from '../../shared/design-system';
import { useBcspI18n } from '../../shared/i18n/runtime';
import { useLocalI18n } from '../i18n/runtime';
import type {
  LocalLocaleOverride,
  LocalSettings,
  PreparedUserDataReset,
  StoredSettings,
} from '../personal/contracts';
import {
  LocalPageFrame,
  completeAction,
  type LocalPageAsyncState,
  type MaybePromise,
} from './PageFrame';

type ConfirmScope = 'FILTERS' | 'VIEWS' | null;
type WorkingScope = 'SETTINGS' | 'FILTERS' | 'VIEWS' | 'PREPARE_ALL' | 'CONFIRM_ALL' | null;

export interface SettingsPageProps extends LocalPageAsyncState {
  readonly onConfirmUserDataReset: (prepared: PreparedUserDataReset) => MaybePromise<void>;
  readonly onDeleteAllSavedViews: () => MaybePromise<void>;
  readonly onPrepareUserDataReset: () => PreparedUserDataReset | Promise<PreparedUserDataReset>;
  readonly onResetCurrentFilters: () => MaybePromise<void>;
  readonly onUpdateSettings: (value: LocalSettings) => MaybePromise<void>;
  readonly savedViewCount: number;
  readonly settings: StoredSettings;
}

function validSettings(value: LocalSettings): boolean {
  return Number.isInteger(value.catalogRefreshMinutes)
    && value.catalogRefreshMinutes >= 1
    && value.catalogRefreshMinutes <= 1440
    && Number.isInteger(value.openRefreshSeconds)
    && value.openRefreshSeconds >= 3
    && value.openRefreshSeconds <= 3600
    && Number.isInteger(value.volumePercent)
    && value.volumePercent >= 0
    && value.volumePercent <= 100
    && Number.isInteger(value.soundPolicy.maxAudible)
    && value.soundPolicy.maxAudible >= 1
    && value.soundPolicy.maxAudible <= 255;
}

function numberValue(value: string): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

export function SettingsPage({
  error,
  onClearError,
  onConfirmUserDataReset,
  onDeleteAllSavedViews,
  onPrepareUserDataReset,
  onReload,
  onResetCurrentFilters,
  onUpdateSettings,
  pending = false,
  savedViewCount,
  settings,
}: SettingsPageProps) {
  const local = useLocalI18n();
  const shared = useBcspI18n();
  const chinese = local.locale === 'zh-CN';
  const localeId = useId();
  const catalogId = useId();
  const openId = useId();
  const volumeId = useId();
  const modeId = useId();
  const maximumId = useId();
  const durationId = useId();
  const [draft, setDraft] = useState<LocalSettings>(settings.value);
  const [working, setWorking] = useState<WorkingScope>(null);
  const [confirmScope, setConfirmScope] = useState<ConfirmScope>(null);
  const [preparedReset, setPreparedReset] = useState<PreparedUserDataReset | null>(null);
  const [saved, setSaved] = useState(false);
  const valid = validSettings(draft);
  const locked = pending || working !== null;

  useEffect(() => {
    setDraft(settings.value);
    setSaved(false);
  }, [settings.revision, settings.value]);

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!valid) return;
    setWorking('SETTINGS');
    setSaved(false);
    const completed = await completeAction(() => onUpdateSettings(draft));
    setWorking(null);
    if (completed) setSaved(true);
  }

  async function confirmSmallReset(scope: Exclude<ConfirmScope, null>) {
    setWorking(scope);
    const completed = await completeAction(
      scope === 'FILTERS' ? onResetCurrentFilters : onDeleteAllSavedViews,
    );
    setWorking(null);
    if (completed) setConfirmScope(null);
  }

  async function prepareFullReset() {
    setWorking('PREPARE_ALL');
    try {
      setPreparedReset(await onPrepareUserDataReset());
    } catch {
      setPreparedReset(null);
    } finally {
      setWorking(null);
    }
  }

  async function confirmFullReset() {
    if (preparedReset === null) return;
    setWorking('CONFIRM_ALL');
    const completed = await completeAction(() => onConfirmUserDataReset(preparedReset));
    setWorking(null);
    if (completed) setPreparedReset(null);
  }

  function updateLocale(localeOverride: LocalLocaleOverride) {
    setSaved(false);
    setDraft((current) => ({ ...current, localeOverride }));
  }

  return (
    <LocalPageFrame
      {...(error === undefined ? {} : { error })}
      intro={local.t('local.settings.intro')}
      kicker={local.t('local.settings.kicker')}
      {...(onClearError === undefined ? {} : { onClearError })}
      {...(onReload === undefined ? {} : { onReload })}
      pending={pending}
      title={local.t('local.settings.title')}
    >
      <section aria-labelledby="local-settings-preferences-title" className="local-personal__section">
        <header className="local-personal__section-head">
          <div>
            <p className="local-personal__kicker">[ 01 / CONTROL ]</p>
            <h4 className="local-personal__section-title" id="local-settings-preferences-title">
              {local.t('local.settings.preferences')}
            </h4>
          </div>
          <span className="local-personal__badge" data-state="READY">
            REV / {local.formatNumber(settings.revision)}
          </span>
        </header>
        <form onSubmit={save}>
          <div className="local-personal__settings">
            <div className="local-personal__field">
              <label htmlFor={localeId}>{local.t('local.settings.locale')}</label>
              <select
                disabled={locked}
                id={localeId}
                onChange={(event) => updateLocale(event.currentTarget.value as LocalLocaleOverride)}
                value={draft.localeOverride}
              >
                <option value="system">{local.t('local.settings.locale_system')}</option>
                <option value="en-US">{local.t('local.settings.locale_en')}</option>
                <option value="zh-CN">{local.t('local.settings.locale_zh')}</option>
              </select>
            </div>
            <div className="local-personal__field">
              <label htmlFor={catalogId}>{local.t('local.settings.catalog_refresh')}</label>
              <input
                disabled={locked}
                id={catalogId}
                inputMode="numeric"
                max={1440}
                min={1}
                onChange={(event) => {
                  setSaved(false);
                  setDraft((current) => ({
                    ...current,
                    catalogRefreshMinutes: numberValue(event.currentTarget.value),
                  }));
                }}
                required
                step={1}
                type="number"
                value={draft.catalogRefreshMinutes}
              />
            </div>
            <div className="local-personal__field">
              <label htmlFor={openId}>{local.t('local.settings.open_refresh')}</label>
              <input
                disabled={locked}
                id={openId}
                inputMode="numeric"
                max={3600}
                min={3}
                onChange={(event) => {
                  setSaved(false);
                  setDraft((current) => ({
                    ...current,
                    openRefreshSeconds: numberValue(event.currentTarget.value),
                  }));
                }}
                required
                step={1}
                type="number"
                value={draft.openRefreshSeconds}
              />
            </div>
            <div className="local-personal__field">
              <label htmlFor={volumeId}>{local.t('local.settings.volume')}</label>
              <div className="local-page__range-row">
                <input
                  disabled={locked}
                  id={volumeId}
                  max={100}
                  min={0}
                  onChange={(event) => {
                    setSaved(false);
                    setDraft((current) => ({
                      ...current,
                      volumePercent: numberValue(event.currentTarget.value),
                    }));
                  }}
                  step={1}
                  type="range"
                  value={draft.volumePercent}
                />
                <output htmlFor={volumeId}>{local.formatNumber(draft.volumePercent)}%</output>
              </div>
            </div>
            <div className="local-personal__field">
              <label htmlFor={modeId}>{local.t('local.settings.mode')}</label>
              <select
                disabled={locked}
                id={modeId}
                onChange={(event) => {
                  const notificationMode = event.currentTarget.value as LocalSettings['soundPolicy']['notificationMode'];
                  setSaved(false);
                  setDraft((current) => ({
                    ...current,
                    soundPolicy: { ...current.soundPolicy, notificationMode },
                  }));
                }}
                value={draft.soundPolicy.notificationMode}
              >
                <option value="ONE_SHOT">{local.t('local.settings.mode_one_shot')}</option>
                <option value="CONTINUOUS">{local.t('local.settings.mode_continuous')}</option>
              </select>
            </div>
            <div className="local-personal__field">
              <label htmlFor={maximumId}>{local.t('local.settings.max_audible')}</label>
              <input
                disabled={locked}
                id={maximumId}
                inputMode="numeric"
                max={255}
                min={1}
                onChange={(event) => {
                  const maxAudible = numberValue(event.currentTarget.value);
                  setSaved(false);
                  setDraft((current) => ({
                    ...current,
                    soundPolicy: { ...current.soundPolicy, maxAudible },
                  }));
                }}
                required
                step={1}
                type="number"
                value={draft.soundPolicy.maxAudible}
              />
            </div>
            <div className="local-personal__field">
              <label htmlFor={durationId}>{local.t('local.settings.duration')}</label>
              <select
                disabled={locked || draft.soundPolicy.notificationMode !== 'CONTINUOUS'}
                id={durationId}
                onChange={(event) => {
                  const continuousDuration: LocalSettings['soundPolicy']['continuousDuration'] =
                    event.currentTarget.value === 'UNLIMITED'
                      ? { kind: 'UNLIMITED' }
                      : { kind: 'FINITE', seconds: 600 };
                  setSaved(false);
                  setDraft((current) => ({
                    ...current,
                    soundPolicy: { ...current.soundPolicy, continuousDuration },
                  }));
                }}
                value={draft.soundPolicy.continuousDuration.kind}
              >
                <option value="FINITE">{local.t('local.settings.duration_finite')}</option>
                <option value="UNLIMITED">{local.t('local.settings.duration_unlimited')}</option>
              </select>
            </div>
          </div>
          <div className="local-page__settings-actions">
            <p className="local-personal__meta">
              {valid
                ? (chinese ? '设置值有效，可保存。' : 'VALUES VALID / READY TO SAVE')
                : (chinese ? '请修正超出范围的设置值。' : 'CORRECT OUT-OF-RANGE VALUES')}
            </p>
            <ActionButton
              busy={working === 'SETTINGS'}
              busyLabel={local.t('local.status.busy')}
              disabled={locked || !valid}
              tone="accent"
              type="submit"
            >
              {local.t('local.settings.save')}
            </ActionButton>
          </div>
          {saved ? <p className="local-page__success" role="status">{local.t('local.settings.saved')}</p> : null}
        </form>
      </section>

      <section aria-labelledby="local-settings-reset-title" className="local-personal__section">
        <header className="local-personal__section-head">
          <div>
            <p className="local-personal__kicker">[ 02 / RESET ]</p>
            <h4 className="local-personal__section-title" id="local-settings-reset-title">
              {local.t('local.reset.title')}
            </h4>
          </div>
          <p>{local.t('local.reset.intro')}</p>
        </header>
        <div className="local-personal__reset-grid">
          <article className="local-personal__card local-page__reset-scope" data-scope="FILTERS">
            <p className="local-personal__meta">01 / FILTERS ONLY</p>
            <h4>{local.t('local.reset.filters_title')}</h4>
            <p>{local.t('local.reset.filters_body')}</p>
            {confirmScope === 'FILTERS' ? (
              <div className="local-page__inline-panel" role="group" aria-label={local.t('local.reset.filters_title')}>
                <p><strong>{chinese ? '确认：' : 'CONFIRM:'}</strong> {local.t('local.reset.filters_body')}</p>
                <div className="local-page__inline-actions">
                  <ActionButton
                    busy={working === 'FILTERS'}
                    busyLabel={local.t('local.status.busy')}
                    onClick={() => void confirmSmallReset('FILTERS')}
                    tone="accent"
                  >
                    {local.t('local.reset.filters_action')}
                  </ActionButton>
                  <ActionButton disabled={locked} onClick={() => setConfirmScope(null)} tone="quiet">
                    {shared.t('action.close')}
                  </ActionButton>
                </div>
              </div>
            ) : (
              <ActionButton disabled={locked} onClick={() => setConfirmScope('FILTERS')} tone="quiet">
                {local.t('local.reset.filters_action')}
              </ActionButton>
            )}
          </article>

          <article className="local-personal__card local-page__reset-scope" data-scope="VIEWS">
            <p className="local-personal__meta">
              02 / {local.t('local.saved.count', { count: local.formatNumber(savedViewCount) })}
            </p>
            <h4>{local.t('local.reset.views_title')}</h4>
            <p>{local.t('local.reset.views_body')}</p>
            {confirmScope === 'VIEWS' ? (
              <div className="local-page__inline-panel" role="group" aria-label={local.t('local.reset.views_title')}>
                <p><strong>{chinese ? '确认：' : 'CONFIRM:'}</strong> {local.t('local.reset.views_body')}</p>
                <div className="local-page__inline-actions">
                  <ActionButton
                    busy={working === 'VIEWS'}
                    busyLabel={local.t('local.status.busy')}
                    onClick={() => void confirmSmallReset('VIEWS')}
                    tone="accent"
                  >
                    {local.t('local.reset.views_action')}
                  </ActionButton>
                  <ActionButton disabled={locked} onClick={() => setConfirmScope(null)} tone="quiet">
                    {shared.t('action.close')}
                  </ActionButton>
                </div>
              </div>
            ) : (
              <ActionButton
                disabled={locked || savedViewCount === 0}
                onClick={() => setConfirmScope('VIEWS')}
                tone="quiet"
              >
                {local.t('local.reset.views_action')}
              </ActionButton>
            )}
          </article>

          <article className="local-personal__card local-page__reset-scope" data-scope="ALL">
            <p className="local-personal__meta">03 / ALL LOCAL USER DATA</p>
            <h4>{local.t('local.reset.user_title')}</h4>
            <p>{local.t('local.reset.user_body')}</p>
            {preparedReset === null ? (
              <ActionButton
                busy={working === 'PREPARE_ALL'}
                busyLabel={local.t('local.status.busy')}
                disabled={locked}
                onClick={() => void prepareFullReset()}
                tone="accent"
              >
                {local.t('local.reset.prepare')}
              </ActionButton>
            ) : (
              <div className="local-page__inline-panel" role="group" aria-label={local.t('local.reset.user_title')}>
                <p>{local.t('local.reset.prepared', {
                  seconds: local.formatNumber(preparedReset.expiresInSeconds),
                })}</p>
                <div className="local-page__inline-actions">
                  <ActionButton
                    busy={working === 'CONFIRM_ALL'}
                    busyLabel={local.t('local.status.busy')}
                    onClick={() => void confirmFullReset()}
                    tone="accent"
                  >
                    {local.t('local.reset.confirm')}
                  </ActionButton>
                  <ActionButton disabled={locked} onClick={() => setPreparedReset(null)} tone="quiet">
                    {shared.t('action.close')}
                  </ActionButton>
                </div>
              </div>
            )}
          </article>
        </div>
      </section>
    </LocalPageFrame>
  );
}
