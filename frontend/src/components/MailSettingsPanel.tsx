import { useEffect, useMemo, useState, type FormEvent } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import { fetchMailConfig, updateMailConfig } from '../api/admin';
import type { ApiError } from '../api/client';
import type {
  MailConfigMeta,
  MailConfigResponse,
  MailConfigUpdatePayload,
  SanitizedMailSenderConfig,
} from '../api/types';
import { classNames } from '../utils/classNames';
import './MailSettingsPanel.css';

type TranslateFn = TFunction<'translation', undefined>;

type FormState = {
  defaultFromEmail: string;
  defaultFromName: string;
  replyToEmail: string;
  replyToName: string;
  apiKey: string;
  sandboxMode: boolean;
  dryRun: boolean;
  overrideRecipient: string;
};

type TemplateIssue = { id: string; summary: string };

type LoadState = 'loading' | 'ready' | 'error';
type SaveState = 'idle' | 'saving' | 'success';

const EMAIL_PATTERN = /^[^@\s]+@[^@\s]+\.[^@\s]+$/i;

const initialFormState: FormState = {
  defaultFromEmail: '',
  defaultFromName: '',
  replyToEmail: '',
  replyToName: '',
  apiKey: '',
  sandboxMode: false,
  dryRun: true,
  overrideRecipient: '',
};

export function MailSettingsPanel() {
  const { t } = useTranslation();
  const [form, setForm] = useState<FormState>(initialFormState);
  const [config, setConfig] = useState<SanitizedMailSenderConfig | null>(null);
  const [meta, setMeta] = useState<MailConfigMeta | null>(null);
  const [loadState, setLoadState] = useState<LoadState>('loading');
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveState, setSaveState] = useState<SaveState>('idle');
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    void loadConfig();
  }, []);

  const templateIssues = useMemo(() => mergeTemplateIssues(config, meta, t), [config, meta, t]);
  const hasTemplateGaps = templateIssues.length > 0;

  useEffect(() => {
    if (hasTemplateGaps && !form.dryRun) {
      setForm((prev) => ({ ...prev, dryRun: true }));
    }
  }, [hasTemplateGaps, form.dryRun]);

  const hasSendgridKey = Boolean(meta?.hasSendgridKey || config?.providers?.sendgrid?.apiKeySet);
  const disabled = loadState !== 'ready' || saveState === 'saving';

  async function loadConfig() {
    setLoadState('loading');
    setLoadError(null);
    try {
      const response = await fetchMailConfig();
      applyConfig(response);
      setLoadState('ready');
    } catch (error) {
      const apiError = error as ApiError;
      setLoadError(apiError.message);
      setLoadState('error');
    }
  }

  const applyConfig = (response: MailConfigResponse) => {
    setConfig(response.config);
    setMeta(response.meta);
    setForm({
      defaultFromEmail: response.config.defaultFrom?.email ?? '',
      defaultFromName: response.config.defaultFrom?.name ?? '',
      replyToEmail: response.config.replyTo?.email ?? '',
      replyToName: response.config.replyTo?.name ?? '',
      apiKey: '',
      sandboxMode: Boolean(response.config.providers?.sendgrid?.sandboxMode),
      dryRun: response.config.testHooks?.dryRun ?? true,
      overrideRecipient: response.config.testHooks?.overrideRecipient ?? '',
    });
    setSaveState('idle');
    setSaveError(null);
  };

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    if (loadState !== 'ready') return;

    const validationMessage = validateForm(form, hasSendgridKey, hasTemplateGaps, t);
    if (validationMessage) {
      setSaveError(validationMessage);
      return;
    }

    setSaveError(null);
    setSaveState('saving');
    try {
      const payload = buildPayload(form, hasTemplateGaps);
      const response = await updateMailConfig(payload);
      applyConfig(response);
      setSaveState('success');
    } catch (error) {
      const apiError = error as ApiError;
      setSaveError(apiError.message);
      setSaveState('idle');
    }
  };

  const handleInputChange = (key: keyof FormState, value: string | boolean) => {
    setForm((prev) => ({
      ...prev,
      [key]: typeof value === 'string' ? value : value,
    }));
  };

  return (
    <section className="mail-settings">
      <header className="mail-settings__header">
        <div>
          <p className="mail-settings__eyebrow">{t('mailSettings.eyebrow')}</p>
          <h3 className="mail-settings__title">{t('mailSettings.title')}</h3>
          <p className="mail-settings__subtitle">{t('mailSettings.subtitle')}</p>
        </div>
        <div className="mail-settings__badges">
          <span className="mail-settings__badge">
            {meta?.source === 'user'
              ? t('mailSettings.status.source.user')
              : t('mailSettings.status.source.example')}
          </span>
          <span
            className={classNames(
              'mail-settings__badge',
              hasSendgridKey ? 'mail-settings__badge--ok' : 'mail-settings__badge--warn',
            )}
          >
            {hasSendgridKey ? t('mailSettings.status.keyPresent') : t('mailSettings.status.keyMissing')}
          </span>
        </div>
      </header>

      {loadState === 'loading' && (
        <div className="mail-settings__status mail-settings__status--info">
          {t('mailSettings.status.loading')}
        </div>
      )}

      {loadState === 'error' && (
        <div className="mail-settings__status mail-settings__status--error">
          <span>{loadError}</span>
          <button type="button" onClick={loadConfig}>
            {t('mailSettings.actions.retry')}
          </button>
        </div>
      )}

      {hasTemplateGaps && (
        <div className="mail-settings__template-alert">
          <div>
            <p className="mail-settings__template-title">{t('mailSettings.templates.warningTitle')}</p>
            <p className="mail-settings__template-subtitle">{t('mailSettings.templates.warningBody')}</p>
            <ul className="mail-settings__template-list">
              {templateIssues.map((issue) => (
                <li key={`${issue.id}-${issue.summary}`}>{issue.summary}</li>
              ))}
            </ul>
          </div>
          <div className="mail-settings__template-note">
            <p>
              {t('mailSettings.templates.cta', {
                root: config?.templateRoot ?? 'templates/email',
              })}
            </p>
          </div>
        </div>
      )}

      <form className="mail-settings__form" onSubmit={handleSubmit}>
        <div className="mail-settings__grid">
          <label className="mail-settings__field">
            <span>{t('mailSettings.fields.defaultFromEmail')}</span>
            <input
              type="email"
              value={form.defaultFromEmail}
              onChange={(event) => handleInputChange('defaultFromEmail', event.target.value)}
              placeholder="alerts@example.edu"
              disabled={disabled}
              required
            />
          </label>
          <label className="mail-settings__field">
            <span>{t('mailSettings.fields.defaultFromName')}</span>
            <input
              type="text"
              value={form.defaultFromName}
              onChange={(event) => handleInputChange('defaultFromName', event.target.value)}
              placeholder={t('mailSettings.fields.defaultFromPlaceholder')}
              disabled={disabled}
            />
          </label>
          <label className="mail-settings__field">
            <span>{t('mailSettings.fields.replyToEmail')}</span>
            <input
              type="email"
              value={form.replyToEmail}
              onChange={(event) => handleInputChange('replyToEmail', event.target.value)}
              placeholder="help@example.edu"
              disabled={disabled}
            />
          </label>
          <label className="mail-settings__field">
            <span>{t('mailSettings.fields.replyToName')}</span>
            <input
              type="text"
              value={form.replyToName}
              onChange={(event) => handleInputChange('replyToName', event.target.value)}
              placeholder={t('mailSettings.fields.replyToPlaceholder')}
              disabled={disabled}
            />
          </label>
        </div>

        <div className="mail-settings__field mail-settings__field--wide">
          <div className="mail-settings__label-row">
            <span>{t('mailSettings.fields.apiKey')}</span>
            {hasSendgridKey && <span className="mail-settings__hint">{t('mailSettings.fields.apiKeySaved')}</span>}
          </div>
          <input
            type="password"
            value={form.apiKey}
            onChange={(event) => handleInputChange('apiKey', event.target.value)}
            placeholder={t('mailSettings.fields.apiKeyPlaceholder')}
            disabled={disabled}
            autoComplete="off"
          />
        </div>

        <div className="mail-settings__toggles">
          <label className="mail-settings__toggle">
            <input
              type="checkbox"
              checked={form.sandboxMode}
              onChange={(event) => handleInputChange('sandboxMode', event.target.checked)}
              disabled={disabled}
            />
            <div>
              <p className="mail-settings__toggle-title">{t('mailSettings.fields.sandboxMode')}</p>
              <p className="mail-settings__toggle-desc">{t('mailSettings.fields.sandboxHint')}</p>
            </div>
          </label>

          <label className="mail-settings__toggle">
            <input
              type="checkbox"
              checked={form.dryRun}
              onChange={(event) => handleInputChange('dryRun', event.target.checked)}
              disabled={disabled || hasTemplateGaps}
            />
            <div>
              <p className="mail-settings__toggle-title">
                {t('mailSettings.fields.dryRun', { locked: hasTemplateGaps })}
              </p>
              <p className="mail-settings__toggle-desc">
                {hasTemplateGaps ? t('mailSettings.templates.blocking') : t('mailSettings.fields.dryRunHint')}
              </p>
            </div>
          </label>
        </div>

        <div className="mail-settings__footer">
          <div className="mail-settings__status-text">
            {meta?.path && <span>{t('mailSettings.status.path', { path: meta.path })}</span>}
            {saveState === 'success' && <span className="mail-settings__status--success">{t('mailSettings.messages.saved')}</span>}
            {saveError && <span className="mail-settings__status--error">{saveError}</span>}
          </div>

          <div className="mail-settings__actions">
            <button type="submit" className="mail-settings__save" disabled={disabled}>
              {saveState === 'saving' ? t('common.status.loading') : t('mailSettings.actions.save')}
            </button>
          </div>
        </div>
      </form>
    </section>
  );
}

function validateForm(
  state: FormState,
  hasSendgridKey: boolean,
  hasTemplateGaps: boolean,
  t: TranslateFn,
): string | null {
  if (!state.defaultFromEmail.trim()) {
    return t('mailSettings.errors.missingFrom');
  }
  if (!EMAIL_PATTERN.test(state.defaultFromEmail.trim())) {
    return t('mailSettings.errors.invalidFrom');
  }
  if (state.replyToEmail.trim() && !EMAIL_PATTERN.test(state.replyToEmail.trim())) {
    return t('mailSettings.errors.invalidReplyTo');
  }
  if (!hasSendgridKey && !state.apiKey.trim()) {
    return t('mailSettings.errors.missingKey');
  }
  if (hasTemplateGaps && state.dryRun === false) {
    return t('mailSettings.errors.templatesBlocking');
  }
  return null;
}

function buildPayload(state: FormState, enforceDryRun: boolean): MailConfigUpdatePayload {
  const trimmedKey = state.apiKey.trim();
  const replyEmail = state.replyToEmail.trim();
  const replyName = state.replyToName.trim();
  const overrideRecipient = state.overrideRecipient.trim();
  const dryRun = enforceDryRun ? true : state.dryRun;

  const payload: MailConfigUpdatePayload = {
    provider: 'sendgrid',
    defaultFrom: {
      email: state.defaultFromEmail.trim(),
      name: state.defaultFromName.trim() || undefined,
    },
    sendgrid: {
      apiKey: trimmedKey || undefined,
      sandboxMode: state.sandboxMode,
    },
    testHooks: {
      dryRun,
      overrideRecipient: overrideRecipient || null,
    },
  };

  if (replyEmail) {
    payload.replyTo = {
      email: replyEmail,
      name: replyName || undefined,
    };
  }

  return payload;
}

function mergeTemplateIssues(
  config: SanitizedMailSenderConfig | null,
  meta: MailConfigMeta | null,
  t: TranslateFn,
): TemplateIssue[] {
  const issues: TemplateIssue[] = [];
  if (meta?.templateIssues?.length) {
    for (const entry of meta.templateIssues) {
      const details = [entry.templateId, entry.locale, entry.kind].filter(Boolean).join(' · ');
      issues.push({
        id: entry.templateId,
        summary:
          entry.message ??
          t('mailSettings.templates.missingEntry', {
            id: entry.templateId,
            locale: entry.locale ?? 'n/a',
            kind: entry.kind ?? 'template',
          }),
      });
    }
  }

  if (!config) return issues;

  const locales = config.supportedLocales ?? [];
  const templates = config.templates ?? {};
  if (!Object.keys(templates).length) {
    issues.push({ id: 'all', summary: t('mailSettings.templates.none') });
    return issues;
  }

  const resolveKindLabel = (kind: 'html' | 'text' | 'subject') =>
    t(`mailSettings.templates.kinds.${kind}` as const);

  for (const [templateId, definition] of Object.entries(templates)) {
    for (const locale of locales) {
      if (!definition.html?.[locale]) {
        issues.push({
          id: templateId,
          summary: t('mailSettings.templates.missingEntry', {
            id: templateId,
            locale,
            kind: resolveKindLabel('html'),
          }),
        });
      }
      if (definition.text && !definition.text[locale]) {
        issues.push({
          id: templateId,
          summary: t('mailSettings.templates.missingEntry', {
            id: templateId,
            locale,
            kind: resolveKindLabel('text'),
          }),
        });
      }
      if (definition.subject && !definition.subject[locale]) {
        issues.push({
          id: templateId,
          summary: t('mailSettings.templates.missingEntry', {
            id: templateId,
            locale,
            kind: resolveKindLabel('subject'),
          }),
        });
      }
    }
  }

  return issues;
}
