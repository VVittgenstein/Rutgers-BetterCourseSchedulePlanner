import {
  useEffect,
  useId,
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
  readonly disabled?: boolean;
  readonly mode?: 'COURSES' | 'SECTIONS';
  readonly validationIssue?: {
    readonly issue: FilterSerializationIssue;
    readonly message: string;
  } | undefined;
}

const VALIDATION_FIELD: Partial<Record<FilterSerializationIssue, keyof FilterStateV1>> = {
  INVALID_AVAILABILITY: 'availability',
  INVALID_CREDIT_RANGE: 'credits',
  INVALID_SECTION_INDEX: 'sectionIndexes',
  INVALID_TEXT: 'text',
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
    case 'text': return state.text?.trim() || null;
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
    case 'courseLocations': return state.courseLocations.length > 0 ? state.courseLocations.join(', ') : null;
    case 'sectionIndexes': return state.sectionIndexes.length > 0 ? state.sectionIndexes.join(', ') : null;
    case 'sectionNumbers': return state.sectionNumbers.length > 0 ? state.sectionNumbers.join(', ') : null;
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
    case 'meetingLocations': return state.meetingLocations.length > 0 ? state.meetingLocations.join(', ') : null;
    case 'buildingRoom': {
      const values = [
        ...state.buildingRoom.buildingCodes.map((code) => `BLDG ${code}`),
        ...state.buildingRoom.roomNumbers.map((number) => `ROOM ${number}`),
      ];
      return values.length > 0 ? values.join(', ') : null;
    }
    case 'examCodes': return state.examCodes.length > 0 ? state.examCodes.join(', ') : null;
    case 'permission': return state.permission === 'ANY' ? null : optionText(state.permission, i18n);
    case 'eligibility': {
      const values = [
        ...state.eligibility.majorCodes.map((value) => `MAJOR ${value}`),
        ...state.eligibility.minorCodes.map((value) => `MINOR ${value}`),
        ...state.eligibility.honorProgramCodes.map((value) => `HONORS ${value}`),
        ...state.eligibility.unitCodes.map((value) => `UNIT ${value}`),
        ...state.eligibility.unitMajors.map(({ unitCode, majorCode }) => `${unitCode}/${majorCode}`),
      ];
      return values.length > 0 ? values.join(', ') : null;
    }
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

function EligibilityControl({
  value,
  onChange,
  disabled,
}: {
  readonly value: FilterStateV1['eligibility'];
  readonly onChange: (value: FilterStateV1['eligibility']) => void;
  readonly disabled: boolean;
}) {
  const i18n = useBcspI18n();
  const [unitDraft, setUnitDraft] = useState('');
  const [majorDraft, setMajorDraft] = useState('');

  const update = <K extends keyof typeof value>(key: K, next: (typeof value)[K]) => {
    onChange({ ...value, [key]: next });
  };
  const addUnitMajor = () => {
    const unitCode = unitDraft.trim();
    const majorCode = majorDraft.trim();
    if (unitCode.length === 0 || majorCode.length === 0) return;
    const next = value.unitMajors.filter((candidate) =>
      candidate.unitCode !== unitCode || candidate.majorCode !== majorCode);
    onChange({ ...value, unitMajors: [...next, { unitCode, majorCode }] });
    setUnitDraft('');
    setMajorDraft('');
  };

  return (
    <div className="filter-panel__eligibility">
      <TokenListControl label={i18n.t('filter.eligibility.major_codes')} values={value.majorCodes} disabled={disabled}
        onChange={(next) => update('majorCodes', next)} />
      <TokenListControl label={i18n.t('filter.eligibility.minor_codes')} values={value.minorCodes} disabled={disabled}
        onChange={(next) => update('minorCodes', next)} />
      <TokenListControl label={i18n.t('filter.eligibility.honors_codes')} values={value.honorProgramCodes} disabled={disabled}
        onChange={(next) => update('honorProgramCodes', next)} />
      <TokenListControl label={i18n.t('filter.eligibility.unit_codes')} values={value.unitCodes} disabled={disabled}
        onChange={(next) => update('unitCodes', next)} />
      <div className="filter-panel__token-control filter-panel__unit-major">
        <span className="filter-panel__sub-label">{i18n.t('filter.eligibility.unit_major_pairs')}</span>
        <div className="filter-panel__input-action filter-panel__input-action--pair">
          <label>
            <span className="bcsp-visually-hidden">{i18n.t('filter.eligibility.unit_code_for_pair')}</span>
            <input className="filter-panel__input" value={unitDraft} placeholder={i18n.t('filter.eligibility.unit_code')}
              disabled={disabled} onChange={(event) => setUnitDraft(event.target.value)} />
          </label>
          <label>
            <span className="bcsp-visually-hidden">{i18n.t('filter.eligibility.major_code_for_pair')}</span>
            <input className="filter-panel__input" value={majorDraft} placeholder={i18n.t('filter.eligibility.major_code')}
              disabled={disabled} onChange={(event) => setMajorDraft(event.target.value)} />
          </label>
          <button className="filter-panel__minor-action" type="button"
            disabled={disabled || unitDraft.trim() === '' || majorDraft.trim() === ''}
            onClick={addUnitMajor}>
            {i18n.t('filter.eligibility.add_pair')}
          </button>
        </div>
        {value.unitMajors.length > 0 ? (
          <ul className="filter-panel__token-list" aria-label={i18n.t('filter.eligibility.unit_major_pairs')}>
            {value.unitMajors.map(({ unitCode, majorCode }, index) => (
              <li className="filter-panel__token" key={`${unitCode}:${majorCode}`}>
                <samp>{unitCode} / {majorCode}</samp>
                <button type="button" disabled={disabled}
                  aria-label={i18n.t('filter.eligibility.remove_pair', { major: majorCode, unit: unitCode })}
                  onClick={() => update('unitMajors', value.unitMajors.filter((_, item) => item !== index))}>
                  ×
                </button>
              </li>
            ))}
          </ul>
        ) : null}
      </div>
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
.filter-panel__chip, .filter-panel__token { display: inline-grid; grid-auto-flow: column; align-items: stretch; border: 1px solid var(--bcsp-line); background: var(--bcsp-paper); font-family: var(--bcsp-font-data); font-size: 0.68rem; }
.filter-panel__chip-label { padding: 0.45rem 0.55rem; overflow-wrap: anywhere; }
.filter-panel__chip-label strong { margin-right: 0.4rem; text-transform: uppercase; }
.filter-panel__chip button, .filter-panel__token button, .filter-panel__window-list button { min-width: 2.2rem; border: 0; border-left: 1px solid var(--bcsp-line); border-radius: 0; color: inherit; background: transparent; cursor: pointer; }
.filter-panel__chip button:hover:not(:disabled), .filter-panel__token button:hover:not(:disabled), .filter-panel__window-list button:hover:not(:disabled) { color: var(--bcsp-accent-ink); background: var(--bcsp-accent); }
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
.filter-panel__minor-action { min-height: 2.75rem; padding: 0.55rem 0.75rem; border: 1px solid var(--bcsp-line); border-left: 0; border-radius: 0; color: var(--bcsp-ink); background: var(--bcsp-paper); font-family: var(--bcsp-font-data); font-size: 0.68rem; font-weight: 700; letter-spacing: 0.07em; text-transform: uppercase; cursor: pointer; }
.filter-panel__minor-action:hover:not(:disabled) { color: var(--bcsp-paper); background: var(--bcsp-ink); }
.filter-panel__minor-action:active:not(:disabled) { transform: translateY(1px); }
.filter-panel__minor-action:disabled { cursor: not-allowed; opacity: 0.5; }
.filter-panel__token-control { display: grid; align-content: start; }
.filter-panel__token-list { margin-top: 0.45rem; }
.filter-panel__token samp { padding: 0.4rem 0.5rem; overflow-wrap: anywhere; }
.filter-panel__checks { display: grid; grid-template-columns: repeat(auto-fit, minmax(10rem, 1fr)); border-top: 1px solid var(--bcsp-line); border-left: 1px solid var(--bcsp-line); }
.filter-panel__check { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 0.55rem; align-items: center; min-height: 2.75rem; padding: 0.55rem; border-right: 1px solid var(--bcsp-line); border-bottom: 1px solid var(--bcsp-line); font-family: var(--bcsp-font-data); font-size: 0.72rem; }
.filter-panel__check input { width: 1rem; height: 1rem; margin: 0; accent-color: var(--bcsp-accent); }
.filter-panel__subject-search { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: var(--bcsp-space-2); align-items: end; }
.filter-panel__subject-count { min-width: 7rem; padding: 0.8rem 0; color: var(--bcsp-ink-muted); font-family: var(--bcsp-font-data); font-size: 0.7rem; text-align: right; }
.filter-panel__subject-list { max-height: 18rem; overflow: auto; border-top: 1px solid var(--bcsp-line); }
.filter-panel__subject-list .filter-panel__checks { border-top: 0; }
.filter-panel__credit-range, .filter-panel__building, .filter-panel__eligibility { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--bcsp-space-2); }
.filter-panel__eligibility { align-items: start; }
.filter-panel__unit-major { grid-column: 1 / -1; }
.filter-panel__availability { display: grid; gap: var(--bcsp-space-2); }
.filter-panel__availability-editor { display: grid; grid-template-columns: 1.2fr 1fr 1fr auto; align-items: end; }
.filter-panel__availability-add { border-left: 0; }
.filter-panel__window-list { display: grid; gap: 1px; padding: 1px; margin: 0; list-style: none; background: var(--bcsp-line); }
.filter-panel__window-list li { display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: stretch; background: var(--bcsp-paper-raised); }
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
  disabled = false,
  mode = 'COURSES',
  validationIssue,
}: FilterPanelProps) {
  const i18n = useBcspI18n();
  const formRef = useRef<HTMLFormElement>(null);
  const [subjectQuery, setSubjectQuery] = useState('');
  const [courseOpen, setCourseOpen] = useState(mode === 'COURSES');
  const [sectionOpen, setSectionOpen] = useState(mode === 'SECTIONS');

  useEffect(() => {
    setCourseOpen(mode === 'COURSES');
    setSectionOpen(mode === 'SECTIONS');
  }, [mode]);

  const fields = useMemo(
    () => [...schema.fields].sort((left, right) => left.chipOrder - right.chipOrder),
    [schema.fields],
  );
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
        row.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }
      row.querySelector<HTMLElement>('input, select, textarea, button')?.focus();
    }, 0);
    return () => window.clearTimeout(timer);
  }, [invalidField]);
  const labelFor = (field: FilterFieldSchemaV1) =>
    isMessageKey(field.i18nKey) ? i18n.t(field.i18nKey) : field.stableId;

  const termOptions = useMemo(() => {
    const terms = new Map<string, string>();
    for (const target of discovery.targets) {
      if (!terms.has(target.key.term)) {
        terms.set(target.key.term, knownText(target.termLabel) ?? target.key.term);
      }
    }
    return [...terms.entries()];
  }, [discovery.targets]);
  const campusOptions = useMemo(() => {
    const campuses = new Map<string, string>();
    for (const target of discovery.targets) {
      if (value.term !== null && target.key.term !== value.term) continue;
      campuses.set(target.key.campus, knownText(target.campusLabel) ?? target.key.campus);
    }
    return [...campuses.entries()];
  }, [discovery.targets, value.term]);
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
        return (
          <select
            className="filter-panel__select"
            aria-label={label}
            value={value.term ?? ''}
            disabled={disabled}
            onChange={(event) => {
              const term = event.target.value || null;
              const availableCampuses = discovery.targets
                .filter((target) => target.key.term === term)
                .map((target) => target.key.campus);
              const retained = value.campuses.filter((campus) => availableCampuses.includes(campus));
              onChange({
                ...value,
                term,
                campuses: retained.length > 0 ? retained : availableCampuses.slice(0, 1),
                subjects: [],
              });
            }}
          >
            <option value="">{i18n.t('filter.term_placeholder')}</option>
            {termOptions.map(([term, termLabel]) => <option key={term} value={term}>{termLabel} / {term}</option>)}
          </select>
        );
      case 'campuses':
        return campusOptions.length === 0
          ? <p className="bcsp-field__helper">{i18n.t('filter.no_campus')}</p>
          : (
            <div className="filter-panel__checks" role="group" aria-label={label}>
              {campusOptions.map(([campus, campusLabel]) => (
                <label className="filter-panel__check" key={campus}>
                  <input
                    type="checkbox"
                    checked={value.campuses.includes(campus)}
                    disabled={disabled}
                    onChange={(event) => {
                      const campuses = toggleValue(value.campuses, campus, event.target.checked);
                      onChange({ ...value, campuses, subjects: [] });
                    }}
                  />
                  <span>{campusLabel} / <samp>{campus}</samp></span>
                </label>
              ))}
            </div>
          );
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
                <p className="bcsp-field__helper">{i18n.t('filter.subject_empty')}</p>
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
      case 'text':
        return <input className="filter-panel__input" aria-label={label} value={value.text ?? ''}
          disabled={disabled} placeholder={i18n.t('search.placeholder')}
          onChange={(event) => update('text', event.target.value || null)} />;
      case 'courseNumbers':
        return <TokenListControl label={i18n.t('filter.course_numbers')} values={value.courseNumbers}
          onChange={(next) => update('courseNumbers', next)} disabled={disabled} placeholder={i18n.t('filter.course_number_placeholder')} />;
      case 'levels':
        return <TokenListControl label={i18n.t('filter.course_levels')} values={value.levels}
          onChange={(next) => update('levels', next)} disabled={disabled} placeholder={i18n.t('filter.level_placeholder')} />;
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
              options={['ANY', 'ALL']} disabled={disabled}
              onChange={(mode) => update('core', { ...value.core, mode })} />
            <TokenListControl label={i18n.t('filter.core_codes')} values={value.core.codes} disabled={disabled}
              onChange={(codes) => update('core', { ...value.core, codes })} placeholder={i18n.t('filter.core_placeholder')} />
          </div>
        );
      case 'prerequisite':
        return <EnumSelect<PrerequisiteFilterV1> label={label} value={value.prerequisite}
          options={['ANY', 'HAS', 'NONE_REPORTED']} disabled={disabled}
          onChange={(next) => update('prerequisite', next)} />;
      case 'courseLocations':
        return <TokenListControl label={i18n.t('filter.course_locations')} values={value.courseLocations}
          onChange={(next) => update('courseLocations', next)} disabled={disabled} />;
      case 'sectionIndexes':
        return <TokenListControl label={i18n.t('filter.section_indexes')} values={value.sectionIndexes}
          onChange={(next) => update('sectionIndexes', next)} disabled={disabled} placeholder={i18n.t('filter.five_digits')} />;
      case 'sectionNumbers':
        return <TokenListControl label={i18n.t('filter.section_numbers')} values={value.sectionNumbers}
          onChange={(next) => update('sectionNumbers', next)} disabled={disabled} />;
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
        return <TokenListControl label={i18n.t('filter.instructor_names')} values={value.instructors}
          onChange={(next) => update('instructors', next)} disabled={disabled} />;
      case 'availability':
        return <AvailabilityControl value={value.availability}
          onChange={(next) => update('availability', next)} disabled={disabled} />;
      case 'meetingLocations':
        return <TokenListControl label={i18n.t('filter.meeting_locations')} values={value.meetingLocations}
          onChange={(next) => update('meetingLocations', next)} disabled={disabled} />;
      case 'buildingRoom':
        return (
          <div className="filter-panel__building">
            <TokenListControl label={i18n.t('filter.building_codes')} values={value.buildingRoom.buildingCodes}
              onChange={(buildingCodes) => update('buildingRoom', { ...value.buildingRoom, buildingCodes })}
              disabled={disabled} />
            <TokenListControl label={i18n.t('filter.room_numbers')} values={value.buildingRoom.roomNumbers}
              onChange={(roomNumbers) => update('buildingRoom', { ...value.buildingRoom, roomNumbers })}
              disabled={disabled} />
          </div>
        );
      case 'examCodes':
        return <TokenListControl label={i18n.t('filter.exam_codes')} values={value.examCodes}
          onChange={(next) => update('examCodes', next)} disabled={disabled} />;
      case 'permission':
        return <EnumSelect<PermissionFilterV1> label={label} value={value.permission}
          options={['ANY', 'REQUIRED', 'NOT_REQUIRED']} disabled={disabled}
          onChange={(next) => update('permission', next)} />;
      case 'eligibility':
        return <EligibilityControl value={value.eligibility}
          onChange={(next) => update('eligibility', next)} disabled={disabled} />;
    }
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const scrollContainer = event.currentTarget.closest<HTMLElement>('.bcsp-search-workspace__filters');
    if (scrollContainer !== null) scrollContainer.scrollTop = 0;
    setCourseOpen(false);
    setSectionOpen(false);
    onSubmit();
  };

  const renderRow = (field: FilterFieldSchemaV1) => {
    const index = fields.indexOf(field);
    const wide = field.requestField === 'subjects'
      || field.requestField === 'availability'
      || field.requestField === 'eligibility';
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
          <span className="filter-panel__ordinal">{String(index + 1).padStart(2, '0')}</span>
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

  const targetFields = fields.filter(({ requestField }) =>
    requestField === 'term' || requestField === 'campuses');
  const courseFields = fields.filter(({ requestField, scope }) =>
    scope === 'COURSE' && requestField !== 'term' && requestField !== 'campuses');
  const sectionFields = fields.filter(({ scope }) => scope === 'SECTION');

  return (
    <>
      <style data-bcsp-filter-panel="">{FILTER_PANEL_CSS}</style>
      <form ref={formRef} className="filter-panel" aria-label={i18n.t('filter.form_label')} onSubmit={submit}>
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
            disabled={disabled || value.term === null || value.campuses.length === 0}>
            {i18n.t('action.search')}
          </button>
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

        <div className="filter-panel__grid">{targetFields.map(renderRow)}</div>
        <details className="filter-panel__group" open={courseOpen}
          onToggle={(event) => {
            const next = event.currentTarget.open;
            setCourseOpen(next);
            if (next) setSectionOpen(false);
          }}>
          <summary className="filter-panel__group-summary">
            <span>03–10</span>
            <span>{i18n.t('filter.course_constraints')}</span>
            <span className="filter-panel__group-count">{courseFields.length}</span>
          </summary>
          <div className="filter-panel__grid">{courseFields.map(renderRow)}</div>
        </details>
        <details className="filter-panel__group" open={sectionOpen}
          onToggle={(event) => {
            const next = event.currentTarget.open;
            setSectionOpen(next);
            if (next) setCourseOpen(false);
          }}>
          <summary className="filter-panel__group-summary">
            <span>11–22</span>
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
      </form>
    </>
  );
}
