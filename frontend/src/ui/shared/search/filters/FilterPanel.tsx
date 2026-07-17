import {
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type ReactNode,
} from 'react';

import { isMessageKey } from '../../i18n/contract';
import {
  filterOptionMessageKey,
  filterSerializationIssueMessageKeys,
} from '../../i18n/presenter';
import { useBcspI18n, type BcspI18nRuntime } from '../../i18n/runtime';
import type {
  CatalogDiscoveryResponseV1,
  CatalogFieldKnowledge,
  CatalogSynchronicity,
  FilterFieldSchemaV1,
  FilterOptionsFieldV2,
  FilterOptionsResponseV2,
  FilterSchemaV1,
  FilterSerializationIssue,
  LiveOpenStateV1,
  ModalityFilterV1,
  PermissionFilterV1,
  PrerequisiteFilterV1,
  WeekdayV1,
} from '../../product';
import {
  createNeutralFilterState,
  type FilterStateV1,
} from '../../product';

export interface FilterPanelProps {
  readonly schema: FilterSchemaV1;
  readonly discovery: CatalogDiscoveryResponseV1;
  readonly value: FilterStateV1;
  readonly onChange: (next: FilterStateV1) => void;
  readonly onSubmit: () => void;
  readonly loadOptions?: (
    field: FilterOptionsFieldV2,
    query?: string,
    signal?: AbortSignal,
  ) => Promise<FilterOptionsResponseV2>;
  readonly disabled?: boolean;
  readonly searchAvailable?: boolean;
  readonly validationIssue?: {
    readonly issue: FilterSerializationIssue;
    readonly message: string;
  } | undefined;
}

const VALIDATION_FIELD: Partial<Record<FilterSerializationIssue, keyof FilterStateV1>> = {
  INVALID_AVAILABILITY: 'availability',
  INVALID_CREDIT_RANGE: 'credits',
  INVALID_SECTION_INDEX: 'sectionIndexes',
  INVALID_TEXT: 'keywords',
  TERM_REQUIRED: 'term',
};

const WEEKDAYS = [
  'MONDAY',
  'TUESDAY',
  'WEDNESDAY',
  'THURSDAY',
  'FRIDAY',
  'SATURDAY',
  'SUNDAY',
] as const satisfies readonly WeekdayV1[];

const OPEN_STATES = ['OPEN', 'CLOSED', 'UNKNOWN'] as const satisfies readonly LiveOpenStateV1[];
const MODALITIES = [
  'ON_CAMPUS_OR_IN_PERSON',
  'ONLINE',
  'HYBRID',
  'OTHER',
  'UNKNOWN',
] as const satisfies readonly ModalityFilterV1[];
const SYNCHRONICITIES = [
  'SYNC',
  'ASYNC',
  'MIXED',
  'UNSPECIFIED',
  'UNKNOWN',
] as const satisfies readonly CatalogSynchronicity[];

const ACTIVE_REQUEST_FIELDS = new Set<keyof FilterStateV1>([
  'term', 'campuses', 'subjects', 'keywords', 'courseNumbers', 'levels', 'credits',
  'core', 'prerequisite', 'sectionIndexes', 'openStatuses', 'modalities',
  'synchronicities', 'instructors', 'availability', 'meetingLocations', 'examCodes',
  'permission',
]);

function knownText(knowledge: CatalogFieldKnowledge<string>): string | null {
  if (knowledge.knowledge !== 'KNOWN' || knowledge.presence.presence !== 'PRESENT') return null;
  return knowledge.presence.value;
}

function optionText(value: string, i18n: BcspI18nRuntime): string {
  const key = filterOptionMessageKey(value);
  return key === undefined ? value : i18n.t(key);
}

function unique<T>(values: readonly T[]): T[] {
  return [...new Set(values)];
}

function toggleValue<T extends string>(values: readonly T[], value: T, checked: boolean): T[] {
  return checked ? unique([...values, value]) : values.filter((candidate) => candidate !== value);
}

function formatCredit(value: number | null): string {
  if (value === null) return '';
  return String(value / 100);
}

function parseCredit(value: string): number | null {
  if (value.trim() === '') return null;
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? Math.round(parsed * 100) : null;
}

function minuteFromTime(value: string): number | null {
  const match = /^(\d{2}):(\d{2})$/u.exec(value);
  if (match === null) return null;
  return Number(match[1]) * 60 + Number(match[2]);
}

function timeFromMinute(value: number): string {
  const hours = Math.floor(value / 60);
  const minutes = value % 60;
  return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}`;
}

function fieldSummary(
  field: FilterFieldSchemaV1,
  state: FilterStateV1,
  i18n: BcspI18nRuntime,
): string | null {
  switch (field.requestField) {
    case 'term': return state.term;
    case 'campuses': return state.campuses.length > 0 ? state.campuses.join(', ') : null;
    case 'subjects': return state.subjects.length > 0 ? state.subjects.join(', ') : null;
    case 'keywords': return state.keywords.length > 0 ? state.keywords.join(', ') : null;
    case 'courseNumbers': return state.courseNumbers.length > 0 ? state.courseNumbers.join(', ') : null;
    case 'levels': return state.levels.length > 0 ? state.levels.join(', ') : null;
    case 'credits': {
      if (state.credits === null) return null;
      const minimum = state.credits.minimumHundredths === null
        ? '−∞'
        : formatCredit(state.credits.minimumHundredths);
      const maximum = state.credits.maximumHundredths === null
        ? '+∞'
        : formatCredit(state.credits.maximumHundredths);
      return `${minimum}–${maximum}`;
    }
    case 'core': return state.core.codes.length > 0
      ? `${state.core.mode}: ${state.core.codes.join(', ')}`
      : null;
    case 'prerequisite': return state.prerequisite === 'ANY' ? null : optionText(state.prerequisite, i18n);
    case 'sectionIndexes': return state.sectionIndexes.length > 0 ? state.sectionIndexes.join(', ') : null;
    case 'openStatuses': return state.openStatuses.length > 0
      ? state.openStatuses.map((entry) => optionText(entry, i18n)).join(', ')
      : null;
    case 'modalities': return state.modalities.length > 0
      ? state.modalities.map((entry) => optionText(entry, i18n)).join(', ')
      : null;
    case 'synchronicities': return state.synchronicities.length > 0
      ? state.synchronicities.map((entry) => optionText(entry, i18n)).join(', ')
      : null;
    case 'instructors': return state.instructors.length > 0 ? state.instructors.join(', ') : null;
    case 'availability': return state.availability.length > 0
      ? state.availability
        .map((window) => `${optionText(window.weekday, i18n)} ${timeFromMinute(window.startMinute)}–${timeFromMinute(window.endMinute)}`)
        .join(', ')
      : null;
    case 'meetingLocations': return state.meetingLocations.locations.length > 0
      ? `${optionText(state.meetingLocations.mode, i18n)}: ${state.meetingLocations.locations.join(', ')}`
      : null;
    case 'examCodes': return state.examCodes.length > 0 ? state.examCodes.join(', ') : null;
    case 'permission': return state.permission === 'ANY' ? null : optionText(state.permission, i18n);
  }
}

function TokenListControl({
  label,
  values,
  onChange,
  disabled,
  placeholder,
}: {
  readonly label: string;
  readonly values: readonly string[];
  readonly onChange: (values: readonly string[]) => void;
  readonly disabled: boolean;
  readonly placeholder?: string;
}) {
  const i18n = useBcspI18n();
  const id = useId();
  const [draft, setDraft] = useState('');

  const add = () => {
    const next = draft.trim();
    if (next.length === 0) return;
    onChange(unique([...values, next]));
    setDraft('');
  };

  return (
    <div className="filter-panel__token-control">
      <label className="filter-panel__sub-label" htmlFor={id}>{label}</label>
      <div className="filter-panel__input-action">
        <input
          id={id}
          className="filter-panel__input"
          value={draft}
          placeholder={placeholder ?? i18n.t('filter.type_value')}
          disabled={disabled}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== 'Enter') return;
            event.preventDefault();
            add();
          }}
        />
        <button
          className="filter-panel__minor-action"
          type="button"
          disabled={disabled || draft.trim().length === 0}
          aria-label={i18n.t('filter.add_value', { label })}
          onClick={add}
        >
          {i18n.t('common.add')}
        </button>
      </div>
      {values.length > 0 ? (
        <ul className="filter-panel__token-list" aria-label={i18n.t('filter.values', { label })}>
          {values.map((value) => (
            <li key={value} className="filter-panel__token">
              <samp>{value}</samp>
              <button
                type="button"
                disabled={disabled}
                aria-label={i18n.t('filter.remove_value', { label, value })}
                onClick={() => onChange(values.filter((candidate) => candidate !== value))}
              >
                ×
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function DictionaryPicker({
  disabled,
  field,
  label,
  loadOptions,
  onChange,
  searchable = false,
  values,
}: {
  readonly disabled: boolean;
  readonly field: FilterOptionsFieldV2;
  readonly label: string;
  readonly loadOptions?: FilterPanelProps['loadOptions'];
  readonly onChange: (values: readonly string[]) => void;
  readonly searchable?: boolean;
  readonly values: readonly string[];
}) {
  const i18n = useBcspI18n();
  const inputId = useId();
  const listId = useId();
  const [query, setQuery] = useState('');
  const [response, setResponse] = useState<FilterOptionsResponseV2 | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);

  useEffect(() => {
    if (disabled || loadOptions === undefined) {
      setResponse(null);
      setLoading(false);
      return undefined;
    }
    setResponse(null);
    setLoading(true);
    const controller = new AbortController();
    const timer = globalThis.setTimeout(() => {
      setFailed(false);
      void loadOptions(field, searchable ? query : undefined, controller.signal)
        .then((next) => {
          if (controller.signal.aborted) return;
          setResponse(next);
          setActiveIndex(0);
        })
        .catch(() => {
          if (!controller.signal.aborted) setFailed(true);
        })
        .finally(() => {
          if (!controller.signal.aborted) setLoading(false);
        });
    }, searchable ? 180 : 0);
    return () => {
      controller.abort();
      globalThis.clearTimeout(timer);
    };
  }, [disabled, field, loadOptions, query, searchable]);

  const options = response?.options ?? [];
  const optionLabel = (value: string, fallback: string) => {
    if (field !== 'COURSE_LEVEL') return fallback;
    if (value === 'U') return i18n.t('filter.level_undergraduate');
    if (value === 'G') return i18n.t('filter.level_graduate');
    return fallback;
  };
  const choose = (next: string) => {
    if (disabled || loading) return;
    onChange(values.includes(next)
      ? values.filter((value) => value !== next)
      : [...values, next]);
    if (searchable) {
      setQuery('');
      setOpen(false);
    }
  };

  return (
    <div className="filter-panel__dictionary">
      {searchable ? (
        <div className="filter-panel__dictionary-input">
          <label className="filter-panel__sub-label" htmlFor={inputId}>{label}</label>
          <input
            aria-activedescendant={open && options[activeIndex] !== undefined
              ? `${listId}-option-${activeIndex}`
              : undefined}
            aria-autocomplete="list"
            aria-controls={listId}
            aria-expanded={open && !disabled}
            aria-haspopup="listbox"
            autoComplete="off"
            className="filter-panel__input"
            disabled={disabled}
            id={inputId}
            onBlur={() => globalThis.setTimeout(() => setOpen(false), 0)}
            onChange={(event) => {
              setQuery(event.target.value);
              setOpen(true);
            }}
            onFocus={() => setOpen(true)}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setOpen(true);
                setActiveIndex((index) => Math.min(index + 1, Math.max(0, options.length - 1)));
              } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                setActiveIndex((index) => Math.max(0, index - 1));
              } else if (event.key === 'Enter' && open && options[activeIndex] !== undefined) {
                event.preventDefault();
                choose(options[activeIndex].value);
              } else if (event.key === 'Escape') {
                setOpen(false);
              }
            }}
            placeholder={i18n.t('filter.dictionary_placeholder')}
            role="combobox"
            type="search"
            value={query}
          />
        </div>
      ) : null}
      <div className="filter-panel__dictionary-options" data-open={!searchable || open || undefined}>
        {loading ? <p className="bcsp-field__helper" role="status">{i18n.t('filter.dictionary_loading')}</p> : null}
        {!loading && failed ? <p className="bcsp-field__helper" role="alert">{i18n.t('filter.dictionary_error')}</p> : null}
        {!loading && !failed && options.length === 0 ? (
          <p className="bcsp-field__helper">{i18n.t('filter.dictionary_empty')}</p>
        ) : null}
        {!loading && !failed && searchable ? (
          <div aria-label={label} aria-multiselectable="true" id={listId} role="listbox">
            {options.map((option, index) => (
              <div
                aria-selected={values.includes(option.value)}
                className="filter-panel__dictionary-option"
                data-active={index === activeIndex || undefined}
                id={`${listId}-option-${index}`}
                key={option.value}
                onMouseDown={(event) => {
                  event.preventDefault();
                  choose(option.value);
                }}
                role="option"
              >
                <span aria-hidden="true">{values.includes(option.value) ? '■' : '□'}</span>
                <span>{optionLabel(option.value, option.label)}</span>
              </div>
            ))}
          </div>
        ) : null}
        {!loading && !failed && !searchable ? (
          <div className="filter-panel__checks" role="group" aria-label={label}>
            {options.map((option) => (
              <label className="filter-panel__check" key={option.value}>
                <input
                  checked={values.includes(option.value)}
                  disabled={disabled}
                  onChange={() => choose(option.value)}
                  type="checkbox"
                  value={option.value}
                />
                <span>{optionLabel(option.value, option.label)}</span>
              </label>
            ))}
          </div>
        ) : null}
      </div>
      {response?.truncated === true ? (
        <p className="bcsp-field__helper">{i18n.t('filter.dictionary_truncated')}</p>
      ) : null}
      {values.length > 0 ? (
        <ul className="filter-panel__token-list" aria-label={i18n.t('filter.values', { label })}>
          {values.map((value) => (
            <li className="filter-panel__token" key={value}>
              <samp>{optionLabel(
                value,
                response?.options.find((option) => option.value === value)?.label ?? value,
              )}</samp>
              <button
                aria-label={i18n.t('filter.remove_value', { label, value })}
                disabled={disabled}
                onClick={() => onChange(values.filter((candidate) => candidate !== value))}
                type="button"
              >×</button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}

function CheckboxSet<T extends string>({
  label,
  options,
  values,
  onChange,
  disabled,
}: {
  readonly label: string;
  readonly options: readonly T[];
  readonly values: readonly T[];
  readonly onChange: (values: readonly T[]) => void;
  readonly disabled: boolean;
}) {
  const i18n = useBcspI18n();
  return (
    <div className="filter-panel__checks" role="group" aria-label={label}>
      {options.map((option) => (
        <label className="filter-panel__check" key={option}>
          <input
            type="checkbox"
            value={option}
            checked={values.includes(option)}
            disabled={disabled}
            onChange={(event) => onChange(toggleValue(values, option, event.target.checked))}
          />
          <span>{optionText(option, i18n)}</span>
        </label>
      ))}
    </div>
  );
}

function EnumSelect<T extends string>({
  label,
  value,
  options,
  onChange,
  disabled,
}: {
  readonly label: string;
  readonly value: T;
  readonly options: readonly T[];
  readonly onChange: (value: T) => void;
  readonly disabled: boolean;
}) {
  const i18n = useBcspI18n();
  const id = useId();
  return (
    <div className="filter-panel__select-control">
      <label className="bcsp-visually-hidden" htmlFor={id}>{label}</label>
      <select
        id={id}
        className="filter-panel__select"
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value as T)}
      >
        {options.map((option) => <option key={option} value={option}>{optionText(option, i18n)}</option>)}
      </select>
    </div>
  );
}

function AvailabilityControl({
  value,
  onChange,
  disabled,
}: {
  readonly value: FilterStateV1['availability'];
  readonly onChange: (value: FilterStateV1['availability']) => void;
  readonly disabled: boolean;
}) {
  const i18n = useBcspI18n();
  const [weekday, setWeekday] = useState<WeekdayV1>('MONDAY');
  const [start, setStart] = useState('09:00');
  const [end, setEnd] = useState('17:00');
  const [error, setError] = useState<string | null>(null);

  const add = () => {
    const startMinute = minuteFromTime(start);
    const endMinute = minuteFromTime(end);
    if (startMinute === null || endMinute === null || startMinute >= endMinute) {
      setError(i18n.t('filter.availability.invalid'));
      return;
    }
    setError(null);
    const identity = `${weekday}:${startMinute}:${endMinute}`;
    const withoutDuplicate = value.filter((window) =>
      `${window.weekday}:${window.startMinute}:${window.endMinute}` !== identity);
    onChange([...withoutDuplicate, { weekday, startMinute, endMinute }]);
  };

  return (
    <div className="filter-panel__availability">
      <div className="filter-panel__availability-editor">
        <label>
          <span className="filter-panel__sub-label">{i18n.t('filter.availability.weekday')}</span>
          <select
            className="filter-panel__select"
            value={weekday}
            disabled={disabled}
            onChange={(event) => setWeekday(event.target.value as WeekdayV1)}
          >
            {WEEKDAYS.map((day) => <option key={day} value={day}>{optionText(day, i18n)}</option>)}
          </select>
        </label>
        <label>
          <span className="filter-panel__sub-label">{i18n.t('filter.availability.start')}</span>
          <input
            className="filter-panel__input"
            type="time"
            value={start}
            disabled={disabled}
            onChange={(event) => setStart(event.target.value)}
          />
        </label>
        <label>
          <span className="filter-panel__sub-label">{i18n.t('filter.availability.end')}</span>
          <input
            className="filter-panel__input"
            type="time"
            value={end}
            disabled={disabled}
            onChange={(event) => setEnd(event.target.value)}
          />
        </label>
        <button
          className="filter-panel__minor-action filter-panel__availability-add"
          type="button"
          disabled={disabled}
          onClick={add}
        >
          {i18n.t('filter.availability.add')}
        </button>
      </div>
      {error === null ? null : <p className="bcsp-field__error" role="alert">{error}</p>}
      {value.length > 0 ? (
        <ol className="filter-panel__window-list" aria-label={i18n.t('filter.availability.list')}>
          {value.map((window, index) => (
            <li key={`${window.weekday}:${window.startMinute}:${window.endMinute}`}>
              <samp>
                {optionText(window.weekday, i18n)} {timeFromMinute(window.startMinute)}–{timeFromMinute(window.endMinute)}
              </samp>
              <button
                type="button"
                disabled={disabled}
                aria-label={i18n.t('filter.availability.remove', { number: index + 1 })}
                onClick={() => onChange(value.filter((_, candidateIndex) => candidateIndex !== index))}
              >
                {i18n.t('common.remove')}
              </button>
            </li>
          ))}
        </ol>
      ) : null}
      <p className="bcsp-field__helper">{i18n.t('filter.availability.helper')}</p>
    </div>
  );
}

export const FILTER_PANEL_CSS = String.raw`
.filter-panel {
  display: grid;
  gap: 0;
  border: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper);
}

.filter-panel__gate {
  display: grid;
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}

.filter-panel__gate:disabled {
  cursor: not-allowed;
}

.filter-panel__head {
  position: sticky;
  top: 0;
  z-index: 1;
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(12rem, auto) auto;
  gap: var(--bcsp-space-3);
  align-items: end;
  padding: var(--bcsp-space-4);
  border-bottom: 3px solid var(--bcsp-line);
  background: var(--bcsp-paper);
}

.filter-panel__kicker,
.filter-panel__ordinal,
.filter-panel__scope,
.filter-panel__sub-label {
  font-family: var(--bcsp-font-data);
  font-size: 0.68rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.filter-panel__kicker { margin: 0 0 var(--bcsp-space-1); color: var(--bcsp-ink-muted); }
.filter-panel__title { margin: 0; font-size: clamp(1.75rem, 4vw, 3.75rem); letter-spacing: -0.055em; line-height: 0.9; text-transform: uppercase; }
.filter-panel__head-note { max-width: 28ch; margin: 0; color: var(--bcsp-ink-muted); font-size: 0.8rem; }
.filter-panel__submit-status {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: 0.65rem;
  margin: 0;
  padding: 0.55rem 0.65rem;
  border-left: 3px solid var(--bcsp-accent);
  color: var(--bcsp-ink-muted);
  background: var(--bcsp-paper-raised);
  font-family: var(--bcsp-font-data);
  font-size: 0.7rem;
  line-height: 1.45;
}

.filter-panel__submit-status strong {
  display: block;
  color: var(--bcsp-ink);
}

.filter-panel__loading-mark {
  width: 0.85rem;
  height: 0.85rem;
  flex: 0 0 auto;
  border: 2px solid var(--bcsp-ink);
  border-top-color: transparent;
  animation: filter-panel-turn 900ms steps(8, end) infinite;
}

@keyframes filter-panel-turn {
  to { transform: rotate(1turn); }
}

.filter-panel__active {
  display: grid;
  grid-template-columns: minmax(7rem, auto) minmax(0, 1fr) auto;
  gap: var(--bcsp-space-2);
  align-items: start;
  padding: var(--bcsp-space-3) var(--bcsp-space-4);
  border-bottom: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
}

.filter-panel__active-title { margin: 0.3rem 0 0; font-family: var(--bcsp-font-data); font-size: 0.7rem; letter-spacing: 0.08em; text-transform: uppercase; }
.filter-panel__chips, .filter-panel__token-list { display: flex; flex-wrap: wrap; gap: 0.35rem; padding: 0; margin: 0; list-style: none; }
.filter-panel__chip, .filter-panel__token { display: inline-grid; min-height: 2.75rem; grid-auto-flow: column; align-items: stretch; border: 1px solid var(--bcsp-line); background: var(--bcsp-paper); font-family: var(--bcsp-font-data); font-size: 0.68rem; }
.filter-panel__chip-label { display: flex; align-items: center; min-height: 2.75rem; padding: 0.45rem 0.55rem; overflow-wrap: anywhere; }
.filter-panel__chip-label strong { margin-right: 0.4rem; text-transform: uppercase; }
.filter-panel__chip button, .filter-panel__token button, .filter-panel__window-list button { min-width: 2.75rem; min-height: 2.75rem; border: 0; border-left: 1px solid var(--bcsp-line); border-radius: 0; color: inherit; background: transparent; cursor: pointer; transition: transform 140ms cubic-bezier(0.23, 1, 0.32, 1); }
.filter-panel__chip button:active:not(:disabled), .filter-panel__token button:active:not(:disabled), .filter-panel__window-list button:active:not(:disabled) { transform: scale(0.97); }
.filter-panel__chip--target { border-color: var(--bcsp-accent); }
.filter-panel__chip-pin { padding: 0.45rem; color: var(--bcsp-ink); border-left: 1px solid var(--bcsp-accent); font-weight: 800; }
.filter-panel__empty { margin: 0.3rem 0 0; color: var(--bcsp-ink-muted); font-size: 0.8rem; }

.filter-panel__grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
.filter-panel__group { margin: 0; border: 0; border-bottom: 1px solid var(--bcsp-line); }
.filter-panel__group[open] { border-bottom: 3px solid var(--bcsp-line); }
.filter-panel__group-summary {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: var(--bcsp-space-2);
  align-items: center;
  min-height: 3.5rem;
  padding: var(--bcsp-space-3) var(--bcsp-space-4);
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
  font-family: var(--bcsp-font-data);
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.07em;
  list-style: none;
  text-transform: uppercase;
  cursor: pointer;
}
.filter-panel__group-summary::-webkit-details-marker { display: none; }
.filter-panel__group-summary::before { content: '+'; color: var(--bcsp-accent); font-size: 1rem; }
.filter-panel__group[open] > .filter-panel__group-summary::before { content: '−'; }
.filter-panel__group-count { color: var(--bcsp-paper-raised); font-variant-numeric: tabular-nums; }
.filter-panel__row { min-width: 0; padding: var(--bcsp-space-3); margin: 0; border: 0; border-bottom: 1px solid var(--bcsp-line); }
.filter-panel__row[data-filter-error='true'] {
  outline: 3px solid var(--bcsp-danger, #b42318);
  outline-offset: -3px;
  background: color-mix(in srgb, var(--bcsp-danger, #b42318) 7%, var(--bcsp-paper));
}
.filter-panel__row:nth-child(odd) { border-right: 1px solid var(--bcsp-line); }
.filter-panel__row--wide { grid-column: 1 / -1; border-right: 0 !important; }
.filter-panel__legend { display: grid; width: 100%; grid-template-columns: auto minmax(0, 1fr) auto; gap: var(--bcsp-space-2); align-items: baseline; padding: 0 0 var(--bcsp-space-2); }
.filter-panel__ordinal { color: var(--bcsp-ink-muted); }
.filter-panel__label { font-weight: 800; letter-spacing: -0.02em; }
.filter-panel__scope { color: var(--bcsp-ink-muted); }
.filter-panel__control { display: grid; gap: var(--bcsp-space-2); }
.filter-panel__validation-error {
  margin: 0;
  padding: 0.55rem 0.65rem;
  border-left: 3px solid var(--bcsp-danger, #b42318);
  color: var(--bcsp-danger, #8f1d14);
  background: var(--bcsp-paper-raised);
  font-family: var(--bcsp-font-data);
  font-size: 0.72rem;
  font-weight: 700;
  line-height: 1.45;
}

.filter-panel__input,
.filter-panel__select {
  width: 100%;
  min-height: 2.75rem;
  padding: 0.65rem 0.75rem;
  border: 1px solid var(--bcsp-line);
  border-radius: 0;
  color: var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
  font-family: var(--bcsp-font-data);
}

.filter-panel__input:disabled, .filter-panel__select:disabled { cursor: not-allowed; opacity: 0.62; }
.filter-panel__sub-label { display: block; margin-bottom: 0.35rem; color: var(--bcsp-ink-muted); }
.filter-panel__input-action { display: grid; grid-template-columns: minmax(0, 1fr) auto; }
.filter-panel__input-action--pair { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto; }
.filter-panel__minor-action { min-height: 2.75rem; padding: 0.55rem 0.75rem; border: 1px solid var(--bcsp-line); border-left: 0; border-radius: 0; color: var(--bcsp-ink); background: var(--bcsp-paper); font-family: var(--bcsp-font-data); font-size: 0.68rem; font-weight: 700; letter-spacing: 0.07em; text-transform: uppercase; cursor: pointer; transition: transform 140ms cubic-bezier(0.23, 1, 0.32, 1); }
.filter-panel__minor-action:active:not(:disabled) { transform: scale(0.97); }
.filter-panel__minor-action:disabled { cursor: not-allowed; opacity: 0.5; }
.filter-panel__token-control { display: grid; align-content: start; }
.filter-panel__token-list { margin-top: 0.45rem; }
.filter-panel__token samp { padding: 0.4rem 0.5rem; overflow-wrap: anywhere; }
.filter-panel__checks { display: grid; grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr)); border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); }
.filter-panel__check { position: relative; display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 0.55rem; align-items: center; min-height: 2.75rem; padding: 0.55rem; border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); font-family: var(--bcsp-font-data); font-size: 0.72rem; cursor: pointer; }
.filter-panel__check input { width: 1rem; height: 1rem; margin: 0; accent-color: var(--bcsp-accent); }
.filter-panel__check:has(input:focus-visible) { z-index: 1; outline: 3px solid var(--bcsp-focus, var(--bcsp-accent)); outline-offset: -3px; }
.filter-panel__check:has(input:disabled) { color: var(--bcsp-ink-muted); cursor: not-allowed; }
.filter-panel__subject-search { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-2); align-items: end; }
.filter-panel__subject-count { min-width: 7rem; padding: 0.8rem 0; color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-data); font-size: 0.7rem; text-align: right; }
.filter-panel__subject-list { max-height: 18rem; overflow: auto; border-top: 1px solid var(--bcsp-line); }
.filter-panel__subject-list .filter-panel__checks { border-top: 0; }
.filter-panel__core-picker { display: grid; gap: var(--bcsp-space-2); }
.filter-panel__dictionary { display: grid; gap: var(--bcsp-space-2); }
.filter-panel__dictionary-input { display: grid; }
.filter-panel__dictionary-options {
  display: none;
  max-height: 15rem;
  overflow: auto;
  border: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
}
.filter-panel__dictionary-options[data-open='true'] { display: grid; }
.filter-panel__dictionary-options > .bcsp-field__helper { padding: var(--bcsp-space-2); }
.filter-panel__dictionary-option {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: var(--bcsp-space-2);
  min-height: 2.75rem;
  align-items: center;
  padding: 0.55rem 0.65rem;
  border-bottom: 1px solid var(--bcsp-line-soft);
  font-family: var(--bcsp-font-data);
  font-size: 0.72rem;
  cursor: pointer;
}
.filter-panel__dictionary-option[aria-selected='true'] {
  color: var(--bcsp-paper-raised);
  background: var(--bcsp-ink);
}
.filter-panel__dictionary-option[data-active='true'] {
  outline: 2px solid var(--bcsp-accent);
  outline-offset: -2px;
}
.filter-panel__incompatible { padding-top: var(--bcsp-space-2); border-top: 1px solid var(--bcsp-line); }
.filter-panel__token--incompatible { border-color: var(--bcsp-accent); color: var(--bcsp-accent); }
.filter-panel__credit-range, .filter-panel__building, .filter-panel__eligibility { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--bcsp-space-2); }
.filter-panel__eligibility { align-items: start; }
.filter-panel__unit-major { grid-column: 1 / -1; }
.filter-panel__availability { display: grid; gap: var(--bcsp-space-2); }
.filter-panel__availability-editor { display: grid; grid-template-columns: 1.2fr 1fr 1fr auto; align-items: end; }
.filter-panel__availability-add { border-left: 0; }
.filter-panel__window-list { display: grid; gap: 1px; padding: 1px; margin: 0; list-style: none; background: var(--bcsp-line); }
.filter-panel__window-list li { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: stretch; background: var(--bcsp-paper-raised); }

@media (hover: hover) and (pointer: fine) {
  .filter-panel__check:hover:not(:has(input:disabled)) { background: var(--bcsp-paper); }
  .filter-panel__chip button:hover:not(:disabled),
  .filter-panel__token button:hover:not(:disabled),
  .filter-panel__window-list button:hover:not(:disabled) { color: var(--bcsp-accent-ink); background: var(--bcsp-accent); }
  .filter-panel__minor-action:hover:not(:disabled) { color: var(--bcsp-paper); background: var(--bcsp-ink); }
  .filter-panel__dictionary-option:hover { color: var(--bcsp-paper-raised); background: var(--bcsp-ink); }
}

@media (prefers-reduced-motion: reduce) {
  .filter-panel__chip button,
  .filter-panel__token button,
  .filter-panel__window-list button,
  .filter-panel__minor-action { transition: none; }
  .filter-panel__chip button:active:not(:disabled),
  .filter-panel__token button:active:not(:disabled),
  .filter-panel__window-list button:active:not(:disabled),
  .filter-panel__minor-action:active:not(:disabled) { transform: none; }
  .filter-panel__loading-mark { animation: none; }
}
.filter-panel__window-list samp { padding: 0.65rem; }
.filter-panel__window-list button { padding: 0.5rem 0.75rem; font-family: var(--bcsp-font-data); font-size: 0.65rem; font-weight: 700; text-transform: uppercase; }
.filter-panel__footer { display: grid; gap: var(--bcsp-space-3); align-items: center; padding: var(--bcsp-space-4); border-top: 3px solid var(--bcsp-line); }
.filter-panel__footer-note { max-width: 55ch; margin: 0; color: var(--bcsp-ink-muted); font-size: 0.78rem; }

@container (max-width: 42rem) {
  .filter-panel__head { grid-template-columns: minmax(0, 1fr) auto; }
  .filter-panel__head-note { grid-column: 1 / -1; }
  .filter-panel__active, .filter-panel__footer { grid-template-columns: 1fr; }
  .filter-panel__grid { grid-template-columns: minmax(0, 1fr); }
  .filter-panel__row, .filter-panel__row:nth-child(odd) { grid-column: 1; border-right: 0; }
  .filter-panel__credit-range, .filter-panel__building, .filter-panel__eligibility { grid-template-columns: minmax(0, 1fr); }
  .filter-panel__unit-major { grid-column: 1; }
  .filter-panel__availability-editor { grid-template-columns: minmax(0, 1fr); gap: var(--bcsp-space-2); }
  .filter-panel__availability-add, .filter-panel__minor-action { border-left: 1px solid var(--bcsp-line); }
  .filter-panel__input-action, .filter-panel__input-action--pair { grid-template-columns: minmax(0, 1fr); gap: 0.35rem; }
  .filter-panel__subject-search { grid-template-columns: 1fr; }
  .filter-panel__subject-count { padding: 0; text-align: left; }
}

@media (max-width: 47.999rem) {
  .filter-panel__head { position: static; }
  .filter-panel__head, .filter-panel__active, .filter-panel__footer { grid-template-columns: 1fr; }
  .filter-panel__grid { grid-template-columns: minmax(0, 1fr); }
  .filter-panel__row, .filter-panel__row:nth-child(odd) { grid-column: 1; border-right: 0; }
  .filter-panel__credit-range, .filter-panel__building, .filter-panel__eligibility { grid-template-columns: minmax(0, 1fr); }
  .filter-panel__unit-major { grid-column: 1; }
  .filter-panel__availability-editor { grid-template-columns: minmax(0, 1fr); gap: var(--bcsp-space-2); }
  .filter-panel__availability-add, .filter-panel__minor-action { border-left: 1px solid var(--bcsp-line); }
  .filter-panel__input-action, .filter-panel__input-action--pair { grid-template-columns: minmax(0, 1fr); gap: 0.35rem; }
  .filter-panel__subject-search { grid-template-columns: 1fr; }
  .filter-panel__subject-count { padding: 0; text-align: left; }
}
`;

export function FilterPanel({
  schema,
  discovery,
  value,
  onChange,
  onSubmit,
  loadOptions,
  disabled = false,
  searchAvailable = true,
  validationIssue,
}: FilterPanelProps) {
  const i18n = useBcspI18n();
  const formRef = useRef<HTMLFormElement>(null);
  const courseSummaryRef = useRef<HTMLElement>(null);
  const sectionSummaryRef = useRef<HTMLElement>(null);
  const pendingGroupFocus = useRef<'COURSE' | 'SECTION' | null>(null);
  const [subjectQuery, setSubjectQuery] = useState('');
  const [courseOpen, setCourseOpen] = useState(true);
  const [sectionOpen, setSectionOpen] = useState(false);

  const fields = useMemo(
    () => [...schema.fields]
      .map((field) => (field.requestField as string) === 'text'
        ? { ...field, requestField: 'keywords' as const }
        : field)
      .filter((field) => ACTIVE_REQUEST_FIELDS.has(field.requestField))
      .filter((field) => field.requestField !== 'term' && field.requestField !== 'campuses')
      .sort((left, right) => left.chipOrder - right.chipOrder),
    [schema.fields],
  );

  useLayoutEffect(() => {
    const pending = pendingGroupFocus.current;
    if (pending === null) return;
    pendingGroupFocus.current = null;
    const summary = pending === 'COURSE' ? courseSummaryRef.current : sectionSummaryRef.current;
    if (summary === null) return;

    summary.focus({ preventScroll: true });
    const filterRail = summary.closest<HTMLElement>('.bcsp-search-workspace__filters');
    const railStyle = filterRail === null ? null : globalThis.getComputedStyle?.(filterRail);
    const railScrolls = filterRail !== null
      && filterRail.scrollHeight > filterRail.clientHeight
      && railStyle !== null
      && /(auto|scroll)/u.test(railStyle.overflowY);
    if (railScrolls && filterRail !== null) {
      const railTop = filterRail.getBoundingClientRect().top;
      const summaryTop = summary.getBoundingClientRect().top;
      const stickyHeaderHeight = formRef.current
        ?.querySelector<HTMLElement>('.filter-panel__head')
        ?.getBoundingClientRect().height ?? 0;
      filterRail.scrollTop += summaryTop - railTop - stickyHeaderHeight;
      return;
    }

    const root = globalThis.document?.documentElement;
    const navigationHeight = Number.parseFloat(root === undefined
      ? '0'
      : globalThis.getComputedStyle(root).getPropertyValue('--bcsp-navigation-height'));
    const top = summary.getBoundingClientRect().top + globalThis.scrollY
      - (Number.isFinite(navigationHeight) ? navigationHeight : 0);
    globalThis.scrollTo?.({ behavior: 'auto', top: Math.max(0, top) });
  }, [courseOpen, sectionOpen]);
  const invalidField = useMemo(() => {
    if (validationIssue === undefined) return undefined;
    const requestField = VALIDATION_FIELD[validationIssue.issue];
    return requestField === undefined
      ? undefined
      : fields.find((field) => field.requestField === requestField);
  }, [fields, validationIssue]);

  useEffect(() => {
    if (invalidField?.scope === 'COURSE') {
      setCourseOpen(true);
      setSectionOpen(false);
    } else if (invalidField?.scope === 'SECTION') {
      setCourseOpen(false);
      setSectionOpen(true);
    }

    if (invalidField === undefined) return undefined;
    const timer = window.setTimeout(() => {
      const row = formRef.current?.querySelector<HTMLElement>(
        `[data-filter-row="${invalidField.stableId}"]`,
      );
      if (row === undefined || row === null) return;
      if (row.scrollIntoView !== undefined) {
        row.scrollIntoView({ behavior: 'auto', block: 'nearest' });
      }
      row.querySelector<HTMLElement>('input, select, textarea, button')?.focus();
    }, 0);
    return () => window.clearTimeout(timer);
  }, [invalidField]);
  const labelFor = (field: FilterFieldSchemaV1) =>
    isMessageKey(field.i18nKey) ? i18n.t(field.i18nKey) : field.stableId;

  const subjectOptions = useMemo(() => {
    const subjects = new Map<string, string>();
    for (const subject of discovery.subjects) {
      if (value.term !== null && subject.target.term !== value.term) continue;
      if (value.campuses.length > 0 && !value.campuses.includes(subject.target.campus)) continue;
      const label = knownText(subject.label);
      subjects.set(subject.code, label === null ? subject.code : `${subject.code} · ${label}`);
    }
    const query = subjectQuery.trim().toLocaleLowerCase(i18n.locale);
    return [...subjects.entries()].filter(([code, label]) =>
      query.length === 0
      || code.toLocaleLowerCase(i18n.locale).includes(query)
      || label.toLocaleLowerCase(i18n.locale).includes(query));
  }, [discovery.subjects, i18n.locale, subjectQuery, value.campuses, value.term]);
  const selectedCoreTargets = useMemo(() => value.term === null
    ? []
    : value.campuses.map((campus) => `${value.term}\u0000${campus}`),
  [value.campuses, value.term]);
  const selectedCoreDictionaries = useMemo(() => {
    const selected = new Set(selectedCoreTargets);
    return (discovery.coreCodeDictionaries ?? []).filter((dictionary) =>
      selected.has(`${dictionary.target.term}\u0000${dictionary.target.campus}`));
  }, [discovery.coreCodeDictionaries, selectedCoreTargets]);
  const coreDictionaryLoading = selectedCoreTargets.length > 0
    && selectedCoreDictionaries.length !== selectedCoreTargets.length;
  const coreDictionary = useMemo(() => {
    const entries = new Map<string, { descriptions: Set<string>; incomplete: boolean }>();
    for (const dictionary of selectedCoreDictionaries) {
      for (const option of dictionary.options) {
        const entry = entries.get(option.code) ?? { descriptions: new Set<string>(), incomplete: false };
        const description = knownText(option.description)?.trim();
        if (description === undefined || description === null || description.length === 0) {
          entry.incomplete = true;
        } else {
          entry.descriptions.add(description);
        }
        entries.set(option.code, entry);
      }
    }
    return [...entries.entries()]
      .sort(([left], [right]) => left.localeCompare(right, 'en-US'))
      .map(([code, entry]) => ({
        code,
        label: !entry.incomplete && entry.descriptions.size === 1
          ? `${code} · ${[...entry.descriptions][0]}`
          : code,
      }));
  }, [selectedCoreDictionaries]);
  const validCoreCodes = useMemo(() => new Set(coreDictionary.map(({ code }) => code)), [coreDictionary]);
  const coreOptions = coreDictionary;
  const incompatibleCoreCodes = coreDictionaryLoading
    ? []
    : value.core.codes.filter((code) => !validCoreCodes.has(code));

  const summaries = fields.map((field) => ({ field, summary: fieldSummary(field, value, i18n) }))
    .filter((entry): entry is { field: FilterFieldSchemaV1; summary: string } => entry.summary !== null);
  const clearable = summaries.filter(({ field }) => field.requestField !== 'term' && field.requestField !== 'campuses');

  const update = <K extends keyof FilterStateV1>(key: K, next: FilterStateV1[K]) => {
    onChange({ ...value, [key]: next });
  };
  const clearField = (field: FilterFieldSchemaV1) => {
    if (field.requestField === 'term' || field.requestField === 'campuses') return;
    const neutral = createNeutralFilterState(value.term);
    onChange({ ...value, [field.requestField]: neutral[field.requestField] } as FilterStateV1);
  };
  const clearAll = () => onChange({
    ...createNeutralFilterState(value.term),
    campuses: [...value.campuses],
  });

  const controlFor = (field: FilterFieldSchemaV1): ReactNode => {
    const label = labelFor(field);
    switch (field.requestField) {
      case 'term':
      case 'campuses':
        return null;
      case 'subjects':
        return (
          <div className="filter-panel__subject-picker">
            <div className="filter-panel__subject-search">
              <label>
                <span className="filter-panel__sub-label">{i18n.t('filter.subject_search')}</span>
                <input
                  className="filter-panel__input"
                  type="search"
                  value={subjectQuery}
                  disabled={disabled}
                  placeholder={i18n.t('filter.subject_placeholder')}
                  onChange={(event) => setSubjectQuery(event.target.value)}
                />
              </label>
              <output className="filter-panel__subject-count" aria-live="polite">
                {i18n.t('filter.subject_count', { count: i18n.formatNumber(subjectOptions.length) })}
              </output>
            </div>
            <div className="filter-panel__subject-list">
              {subjectOptions.length === 0 ? (
                <p className="bcsp-field__helper" role={!searchAvailable ? 'status' : undefined}>
                  {i18n.t(!searchAvailable ? 'filter.apply_scope_first' : 'filter.subject_empty')}
                </p>
              ) : (
                <div className="filter-panel__checks" role="group" aria-label={i18n.t('filter.subject_list')}>
                  {subjectOptions.map(([code, subjectLabel]) => (
                    <label className="filter-panel__check" key={code}>
                      <input
                        type="checkbox"
                        checked={value.subjects.includes(code)}
                        disabled={disabled}
                        onChange={(event) => update('subjects', toggleValue(value.subjects, code, event.target.checked))}
                      />
                      <span>{subjectLabel}</span>
                    </label>
                  ))}
                </div>
              )}
            </div>
          </div>
        );
      case 'keywords':
        return <DictionaryPicker disabled={disabled} field="KEYWORD" label={label}
          loadOptions={loadOptions} searchable values={value.keywords}
          onChange={(next) => update('keywords', next)} />;
      case 'courseNumbers':
        return <TokenListControl label={i18n.t('filter.course_numbers')} values={value.courseNumbers}
          onChange={(next) => update('courseNumbers', next)} disabled={disabled} placeholder={i18n.t('filter.course_number_placeholder')} />;
      case 'levels':
        return <DictionaryPicker disabled={disabled} field="COURSE_LEVEL" label={label}
          loadOptions={loadOptions} values={value.levels}
          onChange={(next) => update('levels', next)} />;
      case 'credits':
        return (
          <div className="filter-panel__credit-range">
            <label><span className="filter-panel__sub-label">{i18n.t('filter.minimum_credits')}</span>
              <input className="filter-panel__input" type="number" min="0" step="0.01"
                value={formatCredit(value.credits?.minimumHundredths ?? null)} disabled={disabled}
                onChange={(event) => {
                  const minimumHundredths = parseCredit(event.target.value);
                  const maximumHundredths = value.credits?.maximumHundredths ?? null;
                  update('credits', minimumHundredths === null && maximumHundredths === null
                    ? null : { minimumHundredths, maximumHundredths });
                }} />
            </label>
            <label><span className="filter-panel__sub-label">{i18n.t('filter.maximum_credits')}</span>
              <input className="filter-panel__input" type="number" min="0" step="0.01"
                value={formatCredit(value.credits?.maximumHundredths ?? null)} disabled={disabled}
                onChange={(event) => {
                  const maximumHundredths = parseCredit(event.target.value);
                  const minimumHundredths = value.credits?.minimumHundredths ?? null;
                  update('credits', minimumHundredths === null && maximumHundredths === null
                    ? null : { minimumHundredths, maximumHundredths });
                }} />
            </label>
          </div>
        );
      case 'core':
        return (
          <div className="filter-panel__control">
            <EnumSelect label={i18n.t('filter.core_mode')} value={value.core.mode}
              options={['ANY', 'ALL']}
              disabled={disabled || coreDictionaryLoading || selectedCoreTargets.length === 0}
              onChange={(mode) => update('core', { ...value.core, mode })} />
            <div className="filter-panel__core-picker">
              {coreDictionaryLoading ? (
                <p className="bcsp-field__helper" role="status">{i18n.t('filter.core_loading')}</p>
              ) : (
                <div className="filter-panel__subject-list">
                  {coreOptions.length === 0 ? (
                    <p className="bcsp-field__helper">{i18n.t('filter.core_empty')}</p>
                  ) : (
                    <div className="filter-panel__checks" role="group" aria-label={i18n.t('filter.core_list')}>
                      {coreOptions.map(({ code, label: optionLabel }) => (
                        <label className="filter-panel__check" key={code}>
                          <input
                            type="checkbox"
                            checked={value.core.codes.includes(code)}
                            disabled={disabled}
                            onChange={(event) => update('core', {
                              ...value.core,
                              codes: toggleValue(value.core.codes, code, event.target.checked),
                            })}
                          />
                          <span>{optionLabel}</span>
                        </label>
                      ))}
                    </div>
                  )}
                </div>
              )}
              {incompatibleCoreCodes.length === 0 ? null : (
                <div className="filter-panel__incompatible">
                  <p className="filter-panel__sub-label">{i18n.t('filter.core_incompatible')}</p>
                  <ul className="filter-panel__token-list">
                    {incompatibleCoreCodes.map((code) => (
                      <li className="filter-panel__token filter-panel__token--incompatible" key={code}>
                        <samp>{code}</samp>
                        <button
                          aria-label={i18n.t('filter.core_incompatible_remove', { code })}
                          disabled={disabled}
                          onClick={() => update('core', {
                            ...value.core,
                            codes: value.core.codes.filter((candidate) => candidate !== code),
                          })}
                          type="button"
                        >×</button>
                      </li>
                    ))}
                  </ul>
                </div>
              )}
            </div>
          </div>
        );
      case 'prerequisite':
        return <EnumSelect<PrerequisiteFilterV1> label={label} value={value.prerequisite}
          options={['ANY', 'HAS', 'NONE_REPORTED']} disabled={disabled}
          onChange={(next) => update('prerequisite', next)} />;
      case 'sectionIndexes':
        return <TokenListControl label={i18n.t('filter.section_indexes')} values={value.sectionIndexes}
          onChange={(next) => update('sectionIndexes', next)} disabled={disabled} placeholder={i18n.t('filter.five_digits')} />;
      case 'openStatuses':
        return <CheckboxSet label={label} options={OPEN_STATES} values={value.openStatuses}
          onChange={(next) => update('openStatuses', next)} disabled={disabled} />;
      case 'modalities':
        return <CheckboxSet label={label} options={MODALITIES} values={value.modalities}
          onChange={(next) => update('modalities', next)} disabled={disabled} />;
      case 'synchronicities':
        return <CheckboxSet label={label} options={SYNCHRONICITIES} values={value.synchronicities}
          onChange={(next) => update('synchronicities', next)} disabled={disabled} />;
      case 'instructors':
        return <DictionaryPicker disabled={disabled} field="INSTRUCTOR" label={label}
          loadOptions={loadOptions} searchable values={value.instructors}
          onChange={(next) => update('instructors', next)} />;
      case 'availability':
        return <AvailabilityControl value={value.availability}
          onChange={(next) => update('availability', next)} disabled={disabled} />;
      case 'meetingLocations':
        return (
          <div className="filter-panel__control">
            <EnumSelect label={i18n.t('filter.meeting_location_mode')} value={value.meetingLocations.mode}
              options={['ANY_MEETING', 'ALL_REQUIRED_MEETINGS']} disabled={disabled}
              onChange={(mode) => update('meetingLocations', { ...value.meetingLocations, mode })} />
            <DictionaryPicker disabled={disabled} field="MEETING_LOCATION" label={label}
              loadOptions={loadOptions} values={value.meetingLocations.locations}
              onChange={(locations) => update('meetingLocations', { ...value.meetingLocations, locations })} />
          </div>
        );
      case 'examCodes':
        return <DictionaryPicker disabled={disabled} field="EXAM_CODE" label={label}
          loadOptions={loadOptions} values={value.examCodes}
          onChange={(next) => update('examCodes', next)} />;
      case 'permission':
        return <EnumSelect<PermissionFilterV1> label={label} value={value.permission}
          options={['ANY', 'REQUIRED', 'NOT_REQUIRED']} disabled={disabled}
          onChange={(next) => update('permission', next)} />;
    }
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (disabled || !searchAvailable || value.term === null || value.campuses.length === 0) return;
    const scrollContainer = event.currentTarget.closest<HTMLElement>('.bcsp-search-workspace__filters');
    if (scrollContainer !== null) scrollContainer.scrollTop = 0;
    setCourseOpen(false);
    setSectionOpen(false);
    onSubmit();
  };

  const renderRow = (field: FilterFieldSchemaV1) => {
    const index = fields.indexOf(field);
    const wide = field.requestField === 'subjects'
      || field.requestField === 'availability';
    const invalid = field.stableId === invalidField?.stableId;
    const errorId = `filter-panel-error-${field.stableId}`;
    return (
      <fieldset
        className={`filter-panel__row${wide ? ' filter-panel__row--wide' : ''}`}
        key={field.stableId}
        data-filter-row={field.stableId}
        data-filter-error={invalid ? 'true' : undefined}
        aria-describedby={invalid ? errorId : undefined}
        aria-invalid={invalid || undefined}
      >
        <legend className="filter-panel__legend">
          <span className="filter-panel__ordinal">{String(index + 3).padStart(2, '0')}</span>
          <span className="filter-panel__label">{labelFor(field)}</span>
          <span className="filter-panel__scope">
            {i18n.t(field.scope === 'COURSE' ? 'filter.scope.course' : 'filter.scope.section')}
          </span>
        </legend>
        <div className="filter-panel__control">
          {controlFor(field)}
          {invalid ? (
            <p className="filter-panel__validation-error" id={errorId} role="alert">
              {validationIssue === undefined
                ? i18n.t('search.validation_body')
                : i18n.t(filterSerializationIssueMessageKeys[validationIssue.issue])}
            </p>
          ) : null}
        </div>
      </fieldset>
    );
  };

  const courseFields = fields.filter(({ requestField, scope }) =>
    scope === 'COURSE' && requestField !== 'term' && requestField !== 'campuses');
  const sectionFields = fields.filter(({ scope }) => scope === 'SECTION');

  return (
    <>
      <style data-bcsp-filter-panel="">{FILTER_PANEL_CSS}</style>
      <form ref={formRef} className="filter-panel" aria-label={i18n.t('filter.form_label')} onSubmit={submit}>
        <fieldset
          aria-busy={disabled && searchAvailable ? true : undefined}
          className="filter-panel__gate"
          disabled={disabled || !searchAvailable}
        >
        <legend className="bcsp-visually-hidden">{i18n.t('filter.form_label')}</legend>
        <header className="filter-panel__head">
          <div>
            <p className="filter-panel__kicker">
              {i18n.t('filter.matrix_kicker', { count: i18n.formatNumber(fields.length) })}
            </p>
            <h2 className="filter-panel__title">{i18n.t('filter.matrix_title')}</h2>
          </div>
          <p className="filter-panel__head-note">
            {i18n.t('filter.matrix_note')}
          </p>
          <button className="bcsp-action bcsp-action--accent" type="submit"
            disabled={disabled || !searchAvailable || value.term === null || value.campuses.length === 0}>
            {i18n.t('action.search')}
          </button>
          {!searchAvailable ? (
            <div className="filter-panel__submit-status" role="status">
              <span>
                <strong>{i18n.t('filter.apply_scope_first')}</strong>
                {i18n.t('filter.apply_scope_detail')}
              </span>
            </div>
          ) : null}
        </header>

        <section className="filter-panel__active" aria-labelledby="active-filter-title">
          <h3 className="filter-panel__active-title" id="active-filter-title">{i18n.t('filter.active_title')}</h3>
          {summaries.length === 0 ? <p className="filter-panel__empty">{i18n.t('filter.active_empty')}</p> : (
            <ul className="filter-panel__chips">
              {summaries.map(({ field, summary }) => {
                const target = field.requestField === 'term' || field.requestField === 'campuses';
                return (
                  <li className={`filter-panel__chip${target ? ' filter-panel__chip--target' : ''}`}
                    key={field.stableId} data-filter-chip={field.stableId}>
                    <span className="filter-panel__chip-label"><strong>{labelFor(field)}</strong>{summary}</span>
                    {target ? <span className="filter-panel__chip-pin" aria-label={i18n.t('filter.target_preserved')}>
                      {i18n.t('filter.target_tag')}
                    </span> : (
                      <button type="button" disabled={disabled}
                        aria-label={i18n.t('filter.clear_one', { label: labelFor(field) })} onClick={() => clearField(field)}>
                        ×
                      </button>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
          <button className="bcsp-action bcsp-action--quiet" type="button"
            disabled={disabled || clearable.length === 0} onClick={clearAll}>
            {i18n.t('action.clear_filters')}
          </button>
        </section>

        <details className="filter-panel__group" open={courseOpen}
          onToggle={(event) => {
            const next = event.currentTarget.open;
            setCourseOpen(next);
            if (next) {
              pendingGroupFocus.current = 'COURSE';
              setSectionOpen(false);
            }
          }}>
          <summary className="filter-panel__group-summary" ref={courseSummaryRef}>
            <span>03–09</span>
            <span>{i18n.t('filter.course_constraints')}</span>
            <span className="filter-panel__group-count">{courseFields.length}</span>
          </summary>
          <div className="filter-panel__grid">{courseFields.map(renderRow)}</div>
        </details>
        <details className="filter-panel__group" open={sectionOpen}
          onToggle={(event) => {
            const next = event.currentTarget.open;
            setSectionOpen(next);
            if (next) {
              pendingGroupFocus.current = 'SECTION';
              setCourseOpen(false);
            }
          }}>
          <summary className="filter-panel__group-summary" ref={sectionSummaryRef}>
            <span>10–18</span>
            <span>{i18n.t('filter.section_constraints')}</span>
            <span className="filter-panel__group-count">{sectionFields.length}</span>
          </summary>
          <div className="filter-panel__grid">{sectionFields.map(renderRow)}</div>
        </details>

        <footer className="filter-panel__footer">
          <p className="filter-panel__footer-note">
            {i18n.t('filter.footer_note')}
          </p>
        </footer>
        </fieldset>
      </form>
    </>
  );
}
