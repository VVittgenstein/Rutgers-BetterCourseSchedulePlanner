import {
  useEffect,
  useId,
  useRef,
  useState,
  type FormEvent,
} from 'react';

import { ActionButton } from '../../shared/design-system';
import { isMessageKey } from '../../shared/i18n/contract';
import { useBcspI18n } from '../../shared/i18n/runtime';
import { useAppRouter } from '../../shared/routing';
import { useLocalI18n } from '../i18n/runtime';
import type { LocalMessageKey } from '../i18n/catalog';
import type {
  SavedViewDefinition,
  SavedViewLibrary,
  SavedViewListItem,
  SavedViewMatch,
  SavedViewReviewReason,
} from '../personal/contracts';
import {
  LocalPageFrame,
  completeAction,
  type LocalPageAsyncState,
  type MaybePromise,
} from './PageFrame';

const MATCH_MESSAGE: Readonly<Record<SavedViewMatch, LocalMessageKey>> = {
  NOT_APPLIED: 'local.saved.match.not_applied',
  CLEAN: 'local.saved.match.clean',
  MODIFIED: 'local.saved.match.modified',
  REVIEW_REQUIRED: 'local.saved.match.review_required',
  INCOMPATIBLE: 'local.saved.match.incompatible',
};

const REVIEW_REASON_MESSAGE: Readonly<Record<SavedViewReviewReason['code'], LocalMessageKey>> = {
  ACTIVE_LEGACY_FILTER: 'local.saved.review.active_legacy_filter',
  UNSUPPORTED_CAMPUS: 'local.saved.review.unsupported_campus',
  SCOPE_UNAVAILABLE: 'local.saved.review.scope_unavailable',
  DYNAMIC_VALUE_UNAVAILABLE: 'local.saved.review.dynamic_value_unavailable',
};

function SavedViewReviewNotice({
  id,
  reasons,
}: {
  readonly id: string;
  readonly reasons: readonly SavedViewReviewReason[];
}) {
  const local = useLocalI18n();
  const shared = useBcspI18n();
  return (
    <div className="local-personal__notice" id={id} role="status">
      <p><strong>{local.t('local.saved.review.intro')}</strong></p>
      <ul>
        {reasons.map((reason, index) => {
          const messageKey = `filter.${reason.stableId.toLowerCase()}`;
          const fieldLabel = isMessageKey(messageKey)
            ? shared.t(messageKey)
            : shared.t('filter.form_label');
          return (
            <li key={`${reason.stableId}-${reason.code}-${index}`}>
              <span title={reason.stableId}>{fieldLabel}</span> — {local.t(REVIEW_REASON_MESSAGE[reason.code])}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

type EditorState =
  | { readonly kind: 'RENAME' | 'DUPLICATE'; readonly id: string; readonly value: string }
  | { readonly kind: 'DELETE'; readonly id: string }
  | null;

export interface SavedViewsPageProps extends LocalPageAsyncState {
  readonly library: SavedViewLibrary;
  readonly onApply: (view: SavedViewDefinition) => MaybePromise<void>;
  readonly onCreate: (name: string) => MaybePromise<void>;
  readonly onDelete: (view: SavedViewDefinition) => MaybePromise<void>;
  readonly onDeleteAll: () => MaybePromise<void>;
  readonly onDuplicate: (view: SavedViewDefinition, name: string) => MaybePromise<void>;
  readonly onRename: (view: SavedViewDefinition, name: string) => MaybePromise<void>;
  readonly onUpdate: (view: SavedViewDefinition) => MaybePromise<void>;
}

interface SavedViewCardProps {
  readonly currentFiltersReady: boolean;
  readonly editor: EditorState;
  readonly item: SavedViewListItem;
  readonly onApply: SavedViewsPageProps['onApply'];
  readonly onDelete: SavedViewsPageProps['onDelete'];
  readonly onDeleteCompleted: () => void;
  readonly onDuplicate: SavedViewsPageProps['onDuplicate'];
  readonly onEdit: (editor: EditorState) => void;
  readonly onRename: SavedViewsPageProps['onRename'];
  readonly onUpdate: SavedViewsPageProps['onUpdate'];
  readonly pending: boolean;
}

function formatStoredTime(
  value: number,
  formatDate: (value: Date | number, options?: Intl.DateTimeFormatOptions) => string,
): string {
  try {
    return formatDate(value, { dateStyle: 'medium', timeStyle: 'short' });
  } catch {
    return String(value);
  }
}

function SavedViewCard({
  currentFiltersReady,
  editor,
  item,
  onApply,
  onDelete,
  onDeleteCompleted,
  onDuplicate,
  onEdit,
  onRename,
  onUpdate,
  pending,
}: SavedViewCardProps) {
  const local = useLocalI18n();
  const shared = useBcspI18n();
  const { navigate } = useAppRouter();
  const inputId = useId();
  const renameTriggerId = useId();
  const duplicateTriggerId = useId();
  const deleteTriggerId = useId();
  const deleteConfirmId = useId();
  const reviewReasonId = useId();
  const focusReturnId = useRef<string | null>(null);
  const previousActiveKind = useRef<Exclude<EditorState, null>['kind'] | null>(null);
  const [working, setWorking] = useState(false);
  const { definition, matchState } = item;
  const activeEditor = editor?.id === definition.id ? editor : null;
  const activeKind = activeEditor?.kind ?? null;
  const editorOpen = editor !== null;
  const disabled = pending || working;
  const compatible = definition.content.status === 'COMPATIBLE';
  const reviewReasons = definition.content.status === 'REVIEW_REQUIRED'
    ? definition.content.reasons
    : null;

  useEffect(() => {
    if (activeKind !== null) {
      document.getElementById(activeKind === 'DELETE' ? deleteConfirmId : inputId)?.focus();
    } else if (previousActiveKind.current !== null && !editorOpen) {
      const trigger = focusReturnId.current === null
        ? null
        : document.getElementById(focusReturnId.current);
      if (trigger instanceof HTMLButtonElement && !trigger.disabled) trigger.focus();
    }
    previousActiveKind.current = activeKind;
  }, [activeKind, deleteConfirmId, editorOpen, inputId]);

  function openEditor(next: Exclude<EditorState, null>, triggerId: string) {
    focusReturnId.current = triggerId;
    onEdit(next);
  }

  async function perform(action: () => MaybePromise<void>, closeEditor = false): Promise<boolean> {
    setWorking(true);
    const completed = await completeAction(action);
    setWorking(false);
    if (completed && closeEditor) onEdit(null);
    return completed;
  }

  async function apply() {
    const completed = await perform(() => onApply(definition));
    if (completed) navigate('/');
  }

  async function deleteView() {
    setWorking(true);
    const completed = await completeAction(() => onDelete(definition));
    setWorking(false);
    if (completed) onDeleteCompleted();
  }

  function submitName(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (activeEditor === null || activeEditor.kind === 'DELETE') return;
    const name = activeEditor.value.trim();
    if (name.length === 0) return;
    void perform(
      () => activeEditor.kind === 'RENAME'
        ? onRename(definition, name)
        : onDuplicate(definition, name),
      true,
    );
  }

  return (
    <li className="local-personal__card">
      <div className="local-personal__identity">
        <div>
          <p className="local-personal__meta">REV / {local.formatNumber(definition.revision)}</p>
          <h4>{definition.name}</h4>
        </div>
        <span className="local-personal__badge" data-state={matchState}>
          {local.t(MATCH_MESSAGE[matchState])}
        </span>
      </div>
      <span className="local-personal__badge" data-state={compatible ? 'READY' : 'DANGER'}>
        {local.t(compatible
          ? 'local.saved.compatible'
          : reviewReasons === null ? 'local.saved.incompatible' : 'local.saved.match.review_required')}
      </span>
      {reviewReasons === null ? null : <SavedViewReviewNotice id={reviewReasonId} reasons={reviewReasons} />}
      <dl className="local-personal__facts">
        <div>
          <dt>{local.t('local.saved.created', { time: '' }).replace(/\s*$/u, '')}</dt>
          <dd>{formatStoredTime(definition.createdAt, local.formatDate)}</dd>
        </div>
        <div>
          <dt>{local.t('local.saved.updated', { time: '' }).replace(/\s*$/u, '')}</dt>
          <dd>{formatStoredTime(definition.updatedAt, local.formatDate)}</dd>
        </div>
      </dl>
      <div className="local-personal__actions">
        <ActionButton
          aria-describedby={reviewReasons === null ? undefined : reviewReasonId}
          aria-label={`${local.t('local.saved.apply')}: ${definition.name}`}
          disabled={disabled || !compatible}
          onClick={() => void apply()}
          tone="accent"
        >
          {local.t('local.saved.apply')}
        </ActionButton>
        <ActionButton
          aria-label={`${local.t('local.saved.rename')}: ${definition.name}`}
          disabled={disabled}
          id={renameTriggerId}
          onClick={() => openEditor(
            { kind: 'RENAME', id: definition.id, value: definition.name },
            renameTriggerId,
          )}
          tone="quiet"
        >
          {local.t('local.saved.rename')}
        </ActionButton>
        <ActionButton
          aria-label={`${local.t('local.saved.update')}: ${definition.name}`}
          disabled={disabled || !currentFiltersReady}
          onClick={() => void perform(() => onUpdate(definition))}
          tone="quiet"
        >
          {local.t('local.saved.update')}
        </ActionButton>
        <ActionButton
          aria-describedby={reviewReasons === null ? undefined : reviewReasonId}
          aria-label={`${local.t('local.saved.duplicate')}: ${definition.name}`}
          disabled={disabled || !compatible}
          id={duplicateTriggerId}
          onClick={() => openEditor(
            { kind: 'DUPLICATE', id: definition.id, value: '' },
            duplicateTriggerId,
          )}
          tone="quiet"
        >
          {local.t('local.saved.duplicate')}
        </ActionButton>
        <ActionButton
          aria-label={`${local.t('local.saved.delete')}: ${definition.name}`}
          disabled={disabled}
          id={deleteTriggerId}
          onClick={() => openEditor(
            { kind: 'DELETE', id: definition.id },
            deleteTriggerId,
          )}
          tone="quiet"
        >
          {local.t('local.saved.delete')}
        </ActionButton>
      </div>
      {activeEditor?.kind === 'DELETE' ? (
        <div className="local-page__inline-panel" role="group" aria-label={`${local.t('local.saved.confirm_delete')}: ${definition.name}`}>
          <p>{local.t('local.saved.confirm_delete')} — <strong>{definition.name}</strong></p>
          <div className="local-page__inline-actions">
            <ActionButton
              busy={working}
              busyLabel={local.t('local.status.busy')}
              id={deleteConfirmId}
              onClick={() => void deleteView()}
              tone="accent"
            >
              {local.t('local.saved.confirm_delete')}
            </ActionButton>
            <ActionButton disabled={working} onClick={() => onEdit(null)} tone="quiet">
              {shared.t('action.close')}
            </ActionButton>
          </div>
        </div>
      ) : activeEditor === null ? null : (
        <form className="local-page__inline-panel" onSubmit={submitName}>
          <label className="local-personal__label" htmlFor={inputId}>
            {local.t(activeEditor.kind === 'RENAME'
              ? 'local.saved.create_name'
              : 'local.saved.duplicate_name')}
          </label>
          <div className="local-page__inline-form">
            <input
              autoFocus
              className="local-page__input"
              disabled={working}
              id={inputId}
              onChange={(event) => onEdit({ ...activeEditor, value: event.currentTarget.value })}
              required
              type="text"
              value={activeEditor.value}
            />
            <ActionButton
              busy={working}
              busyLabel={local.t('local.status.busy')}
              disabled={activeEditor.value.trim().length === 0}
              tone="accent"
              type="submit"
            >
              {local.t(activeEditor.kind === 'RENAME'
                ? 'local.saved.rename'
                : 'local.saved.duplicate')}
            </ActionButton>
          </div>
          <div className="local-page__inline-actions">
            <ActionButton disabled={working} onClick={() => onEdit(null)} tone="quiet">
              {shared.t('action.close')}
            </ActionButton>
          </div>
        </form>
      )}
    </li>
  );
}

export function SavedViewsPage({
  error,
  library,
  onApply,
  onClearError,
  onCreate,
  onDelete,
  onDeleteAll,
  onDuplicate,
  onReload,
  onRename,
  onUpdate,
  pending = false,
}: SavedViewsPageProps) {
  const local = useLocalI18n();
  const shared = useBcspI18n();
  const createNameId = useId();
  const currentReviewReasonId = useId();
  const deleteAllTriggerId = useId();
  const deleteAllConfirmId = useId();
  const [createName, setCreateName] = useState('');
  const [creating, setCreating] = useState(false);
  const [editor, setEditor] = useState<EditorState>(null);
  const [confirmDeleteAll, setConfirmDeleteAll] = useState(false);
  const [deletingAll, setDeletingAll] = useState(false);
  const previousDeleteAllConfirmation = useRef(false);
  const currentFiltersReady = library.currentFilters.value?.content.status === 'COMPATIBLE';
  const currentReviewReasons = library.currentFilters.value?.content.status === 'REVIEW_REQUIRED'
    ? library.currentFilters.value.content.reasons
    : null;

  useEffect(() => {
    if (confirmDeleteAll) {
      document.getElementById(deleteAllConfirmId)?.focus();
    } else if (previousDeleteAllConfirmation.current) {
      const trigger = document.getElementById(deleteAllTriggerId);
      if (trigger instanceof HTMLButtonElement && !trigger.disabled) {
        trigger.focus();
      } else {
        document.getElementById('local-saved-library-title')?.focus();
      }
    }
    previousDeleteAllConfirmation.current = confirmDeleteAll;
  }, [confirmDeleteAll, deleteAllConfirmId, deleteAllTriggerId]);

  async function create(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const name = createName.trim();
    if (name.length === 0 || !currentFiltersReady) return;
    setCreating(true);
    const completed = await completeAction(() => onCreate(name));
    setCreating(false);
    if (completed) setCreateName('');
  }

  async function deleteAll() {
    setDeletingAll(true);
    const completed = await completeAction(onDeleteAll);
    setDeletingAll(false);
    if (completed) setConfirmDeleteAll(false);
  }

  return (
    <LocalPageFrame
      {...(error === undefined ? {} : { error })}
      intro={local.t('local.saved.intro')}
      kicker={local.t('local.saved.kicker')}
      {...(onClearError === undefined ? {} : { onClearError })}
      {...(onReload === undefined ? {} : { onReload })}
      pending={pending}
      title={local.t('local.saved.title')}
    >
      <section aria-labelledby="local-current-filter-title" className="local-personal__section">
        <header className="local-personal__section-head">
          <div>
            <p className="local-personal__kicker">[ 01 / FILTER ]</p>
            <h3 className="local-personal__section-title" id="local-current-filter-title">
              {local.t('local.saved.current')}
            </h3>
          </div>
          <span className="local-personal__badge" data-state={currentFiltersReady ? 'READY' : 'DANGER'}>
            {currentFiltersReady
              ? local.t('local.saved.compatible')
              : local.t('local.saved.match.not_applied')}
          </span>
        </header>
        <p className="local-personal__notice">
          {local.t(currentFiltersReady
            ? 'local.saved.current_ready'
            : currentReviewReasons === null ? 'local.saved.current_empty' : 'local.saved.match.review_required')}
        </p>
        {currentReviewReasons === null
          ? null
          : <SavedViewReviewNotice id={currentReviewReasonId} reasons={currentReviewReasons} />}
        <form className="local-personal__form" onSubmit={create}>
          <div className="local-personal__field">
            <label htmlFor={createNameId}>{local.t('local.saved.create_name')}</label>
            <input
              disabled={pending || creating || !currentFiltersReady}
              id={createNameId}
              onChange={(event) => setCreateName(event.currentTarget.value)}
              placeholder={local.t('local.saved.create_placeholder')}
              required
              type="text"
              value={createName}
            />
          </div>
          <ActionButton
            busy={creating}
            busyLabel={local.t('local.status.busy')}
            disabled={pending || !currentFiltersReady || createName.trim().length === 0}
            tone="accent"
            type="submit"
          >
            {local.t('local.saved.create')}
          </ActionButton>
        </form>
      </section>

      <section aria-labelledby="local-saved-library-title" className="local-personal__section">
        <header className="local-personal__section-head">
          <div>
            <p className="local-personal__kicker">[ 02 / LIBRARY ]</p>
            <h3 className="local-personal__section-title" id="local-saved-library-title" tabIndex={-1}>
              {local.t('local.saved.library')}
            </h3>
          </div>
          <div className="local-personal__actions">
            <span className="local-personal__badge" data-state="READY">
              {local.t('local.saved.count', { count: local.formatNumber(library.views.length) })}
            </span>
            {library.views.length === 0 ? null : (
              <ActionButton
                disabled={pending || deletingAll}
                id={deleteAllTriggerId}
                onClick={() => setConfirmDeleteAll(true)}
                tone="quiet"
              >
                {local.t('local.saved.delete_all')}
              </ActionButton>
            )}
          </div>
        </header>
        {confirmDeleteAll ? (
          <div className="local-personal__notice" role="group" aria-label={local.t('local.saved.confirm_delete_all')}>
            <p><strong>{local.t('local.saved.confirm_delete_all')}</strong></p>
            <p>{local.t('local.saved.delete_all_note')}</p>
            <div className="local-personal__actions">
              <ActionButton
                busy={deletingAll}
                busyLabel={local.t('local.status.busy')}
                id={deleteAllConfirmId}
                onClick={() => void deleteAll()}
                tone="accent"
              >
                {local.t('local.saved.confirm_delete_all')}
              </ActionButton>
              <ActionButton disabled={deletingAll} onClick={() => setConfirmDeleteAll(false)} tone="quiet">
                {shared.t('action.close')}
              </ActionButton>
            </div>
          </div>
        ) : null}
        {library.views.length === 0 ? (
          <p className="local-personal__notice">{local.t('local.saved.empty')}</p>
        ) : (
          <ul className="local-personal__list">
            {library.views.map((item) => (
              <SavedViewCard
                currentFiltersReady={currentFiltersReady}
                editor={editor}
                item={item}
                key={item.definition.id}
                onApply={onApply}
                onDelete={onDelete}
                onDeleteCompleted={() => {
                  setEditor(null);
                  queueMicrotask(() => document.getElementById('local-saved-library-title')?.focus());
                }}
                onDuplicate={onDuplicate}
                onEdit={setEditor}
                onRename={onRename}
                onUpdate={onUpdate}
                pending={pending}
              />
            ))}
          </ul>
        )}
      </section>
    </LocalPageFrame>
  );
}
