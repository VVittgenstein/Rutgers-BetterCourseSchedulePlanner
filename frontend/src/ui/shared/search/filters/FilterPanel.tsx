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

import { isMessageKey, type MessageKey } from '../../i18n/contract';
import {
  filterOptionMessageKey,
  filterSerializationIssueMessageKeys,
} from '../../i18n/presenter';
import { useBcspI18n, type BcspI18nRuntime } from '../../i18n/runtime';
import type {
  CatalogDiscoveryResponseV1,
  CatalogFieldKnowledge,
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
  canonicalCoreCode,
  createNeutralFilterState,
  type FilterStateV1,
} from '../../product';

export interface FilterPanelProps {
  readonly schema: FilterSchemaV1;
  readonly discovery: CatalogDiscoveryResponseV1;
  readonly value: FilterStateV1;
  readonly onChange: (next: FilterStateV1) => void;
  readonly onSubmit: () => void;
  readonly formId?: string | undefined;
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

// UNKNOWN stays available in raw detail evidence, but it is not an ordinary
// HumanTest-facing filter choice.
const OPEN_STATES = ['OPEN', 'CLOSED'] as const satisfies readonly LiveOpenStateV1[];
const MODALITIES = [
  'ON_CAMPUS_OR_IN_PERSON',
  'ONLINE',
  'HYBRID',
] as const satisfies readonly ModalityFilterV1[];
const SYNCHRONICITIES = [
  'SYNC',
  'ASYNC',
  'MIXED',
] as const satisfies FilterStateV1['synchronicities'];
const SYNCHRONICITY_HELP: Readonly<Record<(typeof SYNCHRONICITIES)[number], MessageKey>> = {
  SYNC: 'filter.option_help.sync',
  ASYNC: 'filter.option_help.async',
  MIXED: 'filter.option_help.mixed',
};

/** Spec v2 section 6: the 16 rows are shown as four contiguous groups so the
 * ordinals 03-18 stay ascending. Unknown stable ids fall into the last group. */
const FILTER_GROUPS: readonly {
  readonly key: 'course' | 'requirements' | 'sections' | 'time-place';
  readonly stableIds: readonly string[];
  readonly titleKey: MessageKey;
}[] = [
  { key: 'course', stableIds: ['FLT-C03', 'FLT-C04', 'FLT-C05', 'FLT-C06', 'FLT-C07'], titleKey: 'filter.group.course' },
  { key: 'requirements', stableIds: ['FLT-C08', 'FLT-C09'], titleKey: 'filter.group.requirements' },
  { key: 'sections', stableIds: ['FLT-S01', 'FLT-S03', 'FLT-S04a', 'FLT-S04b', 'FLT-S05'], titleKey: 'filter.group.sections' },
  { key: 'time-place', stableIds: ['FLT-S06', 'FLT-S07', 'FLT-S09', 'FLT-S10'], titleKey: 'filter.group.time_place' },
];

function groupIndexFor(stableId: string): number {
  const index = FILTER_GROUPS.findIndex((group) => group.stableIds.includes(stableId));
  return index === -1 ? FILTER_GROUPS.length - 1 : index;
}

const ACTIVE_REQUEST_FIELDS = new Set<keyof FilterStateV1>([
  'term', 'campuses', 'subjects', 'keywords', 'courseNumberBands', 'levels', 'credits',
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

/** Core codes are case-insensitive: the dictionary publishes `AHo`, the request
 * form stores `AHO`. Toggling on stores exactly one canonical value and drops
 * every other case variant; toggling off drops every case variant. */
function toggleCoreCode(codes: readonly string[], code: string, checked: boolean): string[] {
  const canonical = canonicalCoreCode(code);
  const rest = codes.filter((candidate) => canonicalCoreCode(candidate) !== canonical);
  return checked ? [...rest, canonical] : rest;
}

function hasCoreCode(codes: readonly string[], code: string): boolean {
  const canonical = canonicalCoreCode(code);
  return codes.some((candidate) => canonicalCoreCode(candidate) === canonical);
}

/** Dictionary label for a stored Core code (any case), or the stored code
 * itself when the dictionary does not know it. */
type CoreLabels = ReadonlyMap<string, string>;

function coreCodeLabel(code: string, labels: CoreLabels): string {
  return labels.get(canonicalCoreCode(code)) ?? code;
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

function formatCourseNumberBand(band: number, i18n: BcspI18nRuntime): string {
  return i18n.t('filter.course_number_band_option', {
    band: String(band).padStart(3, '0'),
  });
}

function fieldSummary(
  field: FilterFieldSchemaV1,
  state: FilterStateV1,
  i18n: BcspI18nRuntime,
  coreLabels: CoreLabels,
): string | null {
  switch (field.requestField) {
    case 'term': return state.term;
    case 'campuses': return state.campuses.length > 0 ? state.campuses.join(', ') : null;
    case 'subjects': return state.subjects.length > 0 ? state.subjects.join(', ') : null;
    case 'keywords': return state.keywords.length > 0 ? state.keywords.join(', ') : null;
    case 'courseNumberBands': return state.courseNumberBands.length > 0
      ? state.courseNumberBands.map((band) => formatCourseNumberBand(band, i18n)).join(', ')
      : null;
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
      ? `${state.core.mode}: ${unique(state.core.codes.map((code) => coreCodeLabel(code, coreLabels))).join(', ')}`
      : null;
    case 'prerequisite': return state.prerequisite === 'ANY' ? null : [
      optionText(state.prerequisite, i18n),
      state.includeIncomplete.prerequisite ? i18n.t('filter.include_incomplete_short') : null,
    ].filter(Boolean).join(' / ');
    case 'sectionIndexes': return state.sectionIndexes.length > 0 ? state.sectionIndexes.join(', ') : null;
    case 'openStatuses': return state.openStatuses.length > 0
      ? state.openStatuses.map((entry) => optionText(entry, i18n)).join(', ')
      : null;
    case 'modalities': return state.modalities.length > 0
      ? [
        state.modalities.map((entry) => optionText(entry, i18n)).join(', '),
        state.includeIncomplete.modality ? i18n.t('filter.include_incomplete_short') : null,
      ].filter(Boolean).join(' / ')
      : null;
    case 'synchronicities': return state.synchronicities.length > 0
      ? [
        state.synchronicities.map((entry) => optionText(entry, i18n)).join(', '),
        state.includeIncomplete.synchronicity ? i18n.t('filter.include_incomplete_short') : null,
      ].filter(Boolean).join(' / ')
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
    case 'includeIncomplete': return null;
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
  variant,
}: {
  readonly disabled: boolean;
  readonly field: FilterOptionsFieldV2;
  readonly label: string;
  readonly loadOptions?: FilterPanelProps['loadOptions'];
  readonly onChange: (values: readonly string[]) => void;
  readonly searchable?: boolean;
  readonly values: readonly string[];
  /** Short 2-3 option lists render as choice pills that fill their row. */
  readonly variant?: 'pills' | undefined;
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
    if (field === 'COURSE_NUMBER_BAND') {
      const band = Number(value);
      return Number.isSafeInteger(band) && band >= 0 && band % 100 === 0
        ? formatCourseNumberBand(band, i18n)
        : fallback;
    }
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
              } else if (event.key === 'Home' && open) {
                event.preventDefault();
                setActiveIndex(0);
              } else if (event.key === 'End' && open) {
                event.preventDefault();
                setActiveIndex(Math.max(0, options.length - 1));
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
          <div
            className={variant === 'pills' ? 'filter-panel__checks filter-panel__checks--pills' : 'filter-panel__checks'}
            role="group"
            aria-label={label}
          >
            {options.map((option) => (
              <label className="filter-panel__check" key={option.value}>
                <input
                  aria-label={optionLabel(option.value, option.label)}
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
  help,
  variant,
}: {
  readonly label: string;
  readonly options: readonly T[];
  readonly values: readonly T[];
  readonly onChange: (values: readonly T[]) => void;
  readonly disabled: boolean;
  /** Optional per-option explanation rendered under the option label. */
  readonly help?: Partial<Record<T, MessageKey>> | undefined;
  /** Short 2-3 option lists render as choice pills that fill their row. */
  readonly variant?: 'pills' | undefined;
}) {
  const i18n = useBcspI18n();
  return (
    <div
      className={variant === 'pills' ? 'filter-panel__checks filter-panel__checks--pills' : 'filter-panel__checks'}
      role="group"
      aria-label={label}
    >
      {options.map((option) => {
        const helpKey = help?.[option];
        return (
          <label
            className={helpKey === undefined
              ? 'filter-panel__check'
              : 'filter-panel__check filter-panel__check--explained'}
            key={option}
          >
            <input
              aria-label={optionText(option, i18n)}
              type="checkbox"
              value={option}
              checked={values.includes(option)}
              disabled={disabled}
              onChange={(event) => onChange(toggleValue(values, option, event.target.checked))}
            />
            <span>
              {optionText(option, i18n)}
              {helpKey === undefined ? null : <small>{i18n.t(helpKey)}</small>}
            </span>
          </label>
        );
      })}
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

function PrerequisiteControl({
  disabled,
  includeIncomplete,
  onIncludeIncompleteChange,
  onValueChange,
  value,
}: {
  readonly disabled: boolean;
  readonly includeIncomplete: boolean;
  readonly onIncludeIncompleteChange: (checked: boolean) => void;
  readonly onValueChange: (value: PrerequisiteFilterV1) => void;
  readonly value: PrerequisiteFilterV1;
}) {
  const i18n = useBcspI18n();
  const name = useId();
  const options = ['HAS', 'NONE_REPORTED'] as const;
  return (
    <div className="filter-panel__control">
      <div className="filter-panel__checks" role="radiogroup" aria-label={i18n.t('filter.flt-c09')}>
        {options.map((option) => (
          <label className="filter-panel__check" key={option}>
            <input
              checked={value === option}
              disabled={disabled}
              name={name}
              onChange={() => onValueChange(option)}
              type="radio"
              value={option}
            />
            <span>{optionText(option, i18n)}</span>
          </label>
        ))}
      </div>
      {value === 'ANY' ? null : (
        <button
          className="filter-panel__minor-action filter-panel__clear-choice"
          disabled={disabled}
          onClick={() => onValueChange('ANY')}
          type="button"
        >
          {i18n.t('filter.clear_choice')}
        </button>
      )}
      <IncompleteToggle
        checked={includeIncomplete}
        disabled={disabled}
        onChange={onIncludeIncompleteChange}
      />
    </div>
  );
}

function IncompleteToggle({
  checked,
  disabled,
  helpKey = 'filter.include_incomplete_help',
  onChange,
}: {
  readonly checked: boolean;
  readonly disabled: boolean;
  /** Field-specific explanation; defaults to the generic incomplete-data help. */
  readonly helpKey?: MessageKey;
  readonly onChange: (checked: boolean) => void;
}) {
  const i18n = useBcspI18n();
  return (
    <label className="filter-panel__check filter-panel__incomplete">
      <input
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
        type="checkbox"
      />
      <span>
        <strong>{i18n.t('filter.include_incomplete')}</strong>
        <small>{i18n.t(helpKey)}</small>
      </span>
    </label>
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
          <span className="filter-panel__select-control">
            <select
              className="filter-panel__select"
              value={weekday}
              disabled={disabled}
              onChange={(event) => setWeekday(event.target.value as WeekdayV1)}
            >
              {WEEKDAYS.map((day) => <option key={day} value={day}>{optionText(day, i18n)}</option>)}
            </select>
          </span>
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
/* Quiet Catalog filter rail (spec v2 section 6). Invariants: no overflow on
   .filter-panel, no transition/keyframes here (rail motion lives in
   SEARCH_CONTROL_MOTION_CSS), inner scroll regions hide their scrollbar until
   hover / focus-within (spec 11.2), option groups fill their row (spec 11.1). */
.filter-panel {
  --bcsp-rail-strip-h: 3.25rem;
  display: grid;
  gap: 0;
  min-width: 0;
  background: var(--bcsp-paper-raised);
}

.filter-panel__gate {
  display: grid;
  min-width: 0;
  margin: 0;
  padding: 0;
  border: 0;
}

.filter-panel__gate:disabled { cursor: not-allowed; }

/* Ordinals stay in the DOM for tests and screen readers; the scope tag is gone. */
.filter-panel__ordinal {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
  border: 0;
}

.filter-panel__scope { display: none; }

.filter-panel .bcsp-field__helper,
.filter-panel .bcsp-field__error { margin: 0; }

/* ---- Active conditions strip (sticky inside the rail) ---- */
.filter-panel__active {
  position: sticky;
  top: 0;
  z-index: var(--bcsp-z-rail-sticky);
  display: grid;
  grid-template-columns: auto auto minmax(0, 1fr) auto;
  gap: var(--bcsp-space-1);
  align-items: center;
  min-height: var(--bcsp-rail-strip-h);
  padding: 0.625rem var(--bcsp-space-4);
  border-bottom: 1px solid var(--bcsp-line);
  background: var(--bcsp-paper-raised);
}

.filter-panel__active-title {
  grid-column: 1;
  grid-row: 1;
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-meta);
  text-transform: none;
}

.filter-panel__active::after {
  grid-column: 2;
  grid-row: 1;
  display: inline-flex;
  height: 1.375rem;
  align-items: center;
  padding: 0 0.5rem;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-accent-text);
  background: var(--bcsp-accent-tint);
  font-size: var(--bcsp-text-micro);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  line-height: 1;
  content: attr(data-count);
}

.filter-panel__active[data-count='0']::after { display: none; }

.filter-panel__active > .bcsp-action {
  grid-column: 4;
  grid-row: 1;
  justify-self: end;
}

.filter-panel__chips,
.filter-panel__empty {
  grid-column: 1 / -1;
  grid-row: 2;
}

.filter-panel__empty {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-body);
  line-height: var(--bcsp-lh-body);
}

.filter-panel__chips {
  display: flex;
  flex-wrap: wrap;
  gap: var(--bcsp-space-2);
  max-height: 6rem;
  margin: 0;
  padding: 0 0.5rem 0 0;
  overflow-x: hidden;
  overflow-y: auto;
  list-style: none;
  overscroll-behavior: contain;
  touch-action: pan-y;
  scrollbar-width: none;
  scrollbar-gutter: auto;
}

/* ---- Chips and value tokens ---- */
.filter-panel__token-list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--bcsp-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.filter-panel__chip,
.filter-panel__token {
  position: relative;
  display: inline-flex;
  min-height: 2rem;
  align-items: center;
  gap: 0.25rem;
  padding: 0 0.25rem 0 0.75rem;
  border: 0;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ink-2);
  background: var(--bcsp-surface-2);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
}

.filter-panel__token { min-height: 1.75rem; }

.filter-panel__chip-label {
  min-width: 0;
  padding: 0.25rem 0;
  overflow-wrap: anywhere;
}

.filter-panel__chip-label strong {
  color: var(--bcsp-ink);
  font-weight: 600;
  text-transform: none;
}

.filter-panel__chip-label strong::after { content: ': '; }
[data-bcsp-locale='zh-CN'] .filter-panel__chip-label strong::after { content: '：'; }

.filter-panel__chip button,
.filter-panel__token button {
  position: relative;
  display: inline-flex;
  width: 2rem;
  height: 2rem;
  flex: none;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: 0;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ink-muted);
  background: transparent;
  font-family: var(--bcsp-font-sans);
  font-size: 1.125rem;
  line-height: 1;
  cursor: pointer;
}

.filter-panel__token button {
  width: 1.75rem;
  height: 1.75rem;
}

/* 44px hit box around the 32px glyph: chips keep 12px gaps, so extensions never overlap. */
.filter-panel__chip button::before,
.filter-panel__token button::before,
.filter-panel__window-list button::before {
  position: absolute;
  inset: -0.375rem;
  content: '';
}

.filter-panel__chip button:disabled,
.filter-panel__token button:disabled,
.filter-panel__window-list button:disabled {
  color: var(--bcsp-ink-faint);
  cursor: not-allowed;
}

.filter-panel__chip--target { background: var(--bcsp-accent-tint); }

.filter-panel__chip-pin {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  padding: 0 0.5rem 0 0.25rem;
  color: var(--bcsp-accent-text);
  font-size: var(--bcsp-text-micro);
  font-weight: 600;
  line-height: 1;
}

.filter-panel__chip-pin::before {
  width: 0.375rem;
  height: 0.375rem;
  border-radius: var(--bcsp-radius-pill);
  background: currentColor;
  content: '';
}

.filter-panel__token samp {
  padding: 0.25rem 0;
  font-size: var(--bcsp-text-data);
  font-variant-numeric: tabular-nums;
  overflow-wrap: anywhere;
}

.filter-panel__token--incompatible {
  border: 1px solid var(--bcsp-danger-line);
  color: var(--bcsp-danger);
  background: var(--bcsp-danger-tint);
}

/* ---- Four groups with sticky heads ---- */
.filter-panel__grid {
  display: grid;
  gap: var(--bcsp-space-3);
  padding: 0 0 var(--bcsp-space-1);
}

.filter-panel__group {
  display: grid;
  min-width: 0;
}

.filter-panel__group-head {
  position: sticky;
  top: var(--bcsp-rail-strip-h);
  z-index: var(--bcsp-z-sticky-sub);
  display: flex;
  height: 2.5rem;
  align-items: center;
  justify-content: space-between;
  gap: var(--bcsp-space-2);
  padding: 0 var(--bcsp-space-4);
  border-block: 1px solid var(--bcsp-line-soft);
  background: var(--bcsp-surface-2);
}

.filter-panel__group-title {
  margin: 0;
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-subtitle);
  font-weight: 600;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-subtitle);
  text-transform: none;
}

.filter-panel__group-count {
  display: inline-flex;
  height: 1.375rem;
  align-items: center;
  padding: 0 0.5rem;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-accent-text);
  background: var(--bcsp-accent-tint);
  font-size: var(--bcsp-text-micro);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  line-height: 1;
}

.filter-panel__group[data-count='0'] .filter-panel__group-count { display: none; }

.filter-panel__group-body {
  display: grid;
  min-width: 0;
  padding: 0 var(--bcsp-space-4) var(--bcsp-space-1);
}

/* ---- Row anatomy: single column, hairline separated ---- */
.filter-panel__row {
  position: relative;
  min-width: 0;
  margin: 0;
  padding: 0.625rem 0 0.875rem;
  border: 0;
  border-bottom: 1px solid var(--bcsp-line-soft);
  scroll-margin-top: calc(var(--bcsp-rail-strip-h) + 3rem);
}

.filter-panel__group-body > .filter-panel__row:last-child { border-bottom: 0; }

.filter-panel__row[data-filter-error='true'] {
  margin-inline: -0.75rem;
  padding-inline: 0.75rem;
  border-radius: 0.5rem;
  background: var(--bcsp-danger-tint);
}

.filter-panel__legend {
  display: flex;
  width: 100%;
  align-items: baseline;
  gap: var(--bcsp-space-2);
  padding: 0 0 var(--bcsp-space-1);
}

.filter-panel__label {
  color: var(--bcsp-ink);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1.25rem;
}

.filter-panel__control {
  display: grid;
  gap: var(--bcsp-space-2);
  min-width: 0;
}

.filter-panel__validation-error {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.5rem;
  align-items: start;
  margin: 0;
  color: var(--bcsp-danger);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  line-height: var(--bcsp-lh-meta);
}

.filter-panel__validation-error::before {
  display: inline-flex;
  width: 1rem;
  height: 1rem;
  align-items: center;
  justify-content: center;
  margin-top: 0.0625rem;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-accent-ink);
  background: var(--bcsp-danger);
  font-size: 0.6875rem;
  font-weight: 600;
  line-height: 1;
  content: '!';
}

/* ---- Inputs and selects ---- */
.filter-panel__input,
.filter-panel__select {
  width: 100%;
  height: var(--bcsp-control-h);
  padding: 0 0.75rem;
  border: 1px solid var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  line-height: 1.25rem;
}

.filter-panel__input::placeholder { color: var(--bcsp-ink-faint); opacity: 1; }

.filter-panel__input:focus-visible,
.filter-panel__select:focus-visible {
  border-color: var(--bcsp-focus);
  outline-offset: 0;
}

.filter-panel__input:disabled,
.filter-panel__select:disabled {
  color: var(--bcsp-ink-muted);
  background: var(--bcsp-surface-3);
  cursor: not-allowed;
  opacity: 1;
}

[data-filter-error='true'] .filter-panel__input,
.filter-panel__input[aria-invalid='true'] { border-color: var(--bcsp-danger); }

.filter-panel__select-control {
  position: relative;
  display: block;
  min-width: 0;
}

.filter-panel__select {
  appearance: none;
  padding-right: 2.25rem;
}

.filter-panel__select-control::after {
  position: absolute;
  top: 50%;
  right: 0.875rem;
  width: 0.375rem;
  height: 0.375rem;
  border-right: 1.5px solid currentColor;
  border-bottom: 1.5px solid currentColor;
  transform: translateY(-70%) rotate(45deg);
  pointer-events: none;
  content: '';
}

/* Search-style inputs: the magnifier is drawn on the wrapper, no image assets. */
.filter-panel__dictionary-input,
.filter-panel__subject-search > label {
  position: relative;
  display: block;
  min-width: 0;
}

.filter-panel__dictionary-input .filter-panel__input,
.filter-panel__subject-search .filter-panel__input { padding-left: 2.25rem; }

.filter-panel__dictionary-input::before,
.filter-panel__subject-search > label::before {
  position: absolute;
  bottom: 1.125rem;
  left: 0.875rem;
  width: 0.5rem;
  height: 0.5rem;
  border: 1.5px solid var(--bcsp-ink-muted);
  border-radius: var(--bcsp-radius-pill);
  pointer-events: none;
  content: '';
}

.filter-panel__dictionary-input::after,
.filter-panel__subject-search > label::after {
  position: absolute;
  bottom: 0.9375rem;
  left: 1.4375rem;
  width: 0.3125rem;
  height: 1.5px;
  background: var(--bcsp-ink-muted);
  transform: rotate(45deg);
  pointer-events: none;
  content: '';
}

.filter-panel__sub-label {
  display: block;
  margin-bottom: 0.25rem;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  letter-spacing: 0;
  line-height: var(--bcsp-lh-meta);
  text-transform: none;
}

.filter-panel__input-action {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
}

.filter-panel__input-action .filter-panel__input {
  border-top-right-radius: 0;
  border-bottom-right-radius: 0;
}

.filter-panel__input-action .filter-panel__minor-action {
  margin-left: -1px;
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
}

.filter-panel__minor-action {
  display: inline-flex;
  min-height: var(--bcsp-control-h);
  align-items: center;
  justify-content: center;
  gap: var(--bcsp-space-1);
  padding: 0 0.75rem;
  border: 1px solid var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-2);
  color: var(--bcsp-ink);
  background: var(--bcsp-paper-raised);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  font-weight: 600;
  letter-spacing: 0;
  line-height: 1.25rem;
  text-transform: none;
  white-space: nowrap;
  cursor: pointer;
}

.filter-panel__minor-action:disabled {
  color: var(--bcsp-ink-muted);
  border-color: var(--bcsp-line);
  background: var(--bcsp-surface-3);
  cursor: not-allowed;
}

.filter-panel__clear-choice {
  justify-self: start;
  border-color: transparent;
  color: var(--bcsp-ink-2);
  background: transparent;
}

.filter-panel__token-control {
  display: grid;
  gap: var(--bcsp-space-2);
  align-content: start;
}

/* ---- Option rows: flex wrap with growing items so every row fills its width ---- */
.filter-panel__checks {
  display: flex;
  flex-wrap: wrap;
  gap: var(--bcsp-space-1);
  min-width: 0;
}

.filter-panel__check {
  position: relative;
  display: flex;
  flex: 1 1 9rem;
  min-width: 0;
  min-height: var(--bcsp-control-h);
  align-items: center;
  gap: 0.625rem;
  padding: 0.375rem 0.625rem;
  border-radius: var(--bcsp-radius-2);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  line-height: 1.25rem;
  cursor: pointer;
}

.filter-panel__check input {
  width: 1.125rem;
  height: 1.125rem;
  flex: none;
  margin: 0;
  accent-color: var(--bcsp-accent);
}

.filter-panel__check > span {
  min-width: 0;
  overflow-wrap: anywhere;
}

.filter-panel__check input:checked + span { font-weight: 600; }
.filter-panel__check:has(input:checked) { background: var(--bcsp-surface-selected); }

.filter-panel__check:has(input:focus-visible) {
  z-index: 1;
  outline: 2px solid var(--bcsp-focus);
  outline-offset: -2px;
}

.filter-panel__check:has(input:disabled) {
  color: var(--bcsp-ink-muted);
  cursor: not-allowed;
}

.filter-panel__check--explained > span {
  display: grid;
  gap: 0.125rem;
}

.filter-panel__check--explained small {
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-meta);
  font-weight: 400;
  line-height: var(--bcsp-lh-meta);
}

/* Choice pills (fields 06 / 11 / 12 / 13): borderless surface-2 pills, checked -> accent tint. */
.filter-panel__checks--pills .filter-panel__check {
  flex: 1 1 7rem;
  padding: 0.375rem 0.875rem;
  border: 1px solid transparent;
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-surface-2);
}

.filter-panel__checks--pills .filter-panel__check:has(input:checked) {
  border-color: var(--bcsp-accent-tint-strong);
  color: var(--bcsp-ink);
  background: var(--bcsp-accent-tint);
}

/* Radio cards (prerequisites): two per row, tinted when chosen. */
.filter-panel__checks[role='radiogroup'] .filter-panel__check {
  flex: 1 1 10rem;
  align-items: flex-start;
  padding: 0.75rem 0.875rem;
  border: 1px solid var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-3);
}

.filter-panel__checks[role='radiogroup'] .filter-panel__check input { margin-top: 0.0625rem; }

.filter-panel__checks[role='radiogroup'] .filter-panel__check:has(input:checked) {
  border-color: var(--bcsp-accent-text);
  color: var(--bcsp-ink);
  background: var(--bcsp-accent-tint);
}

/* Switch-style row ("Complete data display"). */
.filter-panel__incomplete {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.75rem;
  align-items: center;
  min-height: var(--bcsp-control-h);
  padding: 0.5rem 0.75rem;
  border-radius: var(--bcsp-radius-2);
  background: var(--bcsp-surface-2);
}

.filter-panel__incomplete:has(input:checked) { background: var(--bcsp-surface-selected); }

.filter-panel__incomplete > span {
  display: grid;
  gap: 0.125rem;
}

.filter-panel__incomplete strong {
  font-size: 0.8125rem;
  font-weight: 600;
  line-height: 1.125rem;
}

.filter-panel__incomplete small {
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
}

/* ---- Subject / core lists: 44px rows inside a scroll region ---- */
.filter-panel__subject-picker {
  display: grid;
  gap: var(--bcsp-space-2);
  min-width: 0;
}

.filter-panel__subject-search {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: var(--bcsp-space-2);
  align-items: end;
}

.filter-panel__subject-count {
  display: flex;
  min-height: var(--bcsp-control-h);
  align-items: center;
  color: var(--bcsp-ink-muted);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-variant-numeric: tabular-nums;
  line-height: var(--bcsp-lh-meta);
  text-align: right;
}

.filter-panel__subject-list {
  max-height: 12rem;
  padding-right: 0.5rem;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  touch-action: pan-y;
  scrollbar-width: none;
  scrollbar-gutter: auto;
}

.filter-panel__subject-list .filter-panel__check > span { font-variant-numeric: tabular-nums; }

.filter-panel__core-picker {
  display: grid;
  gap: var(--bcsp-space-2);
  min-width: 0;
}

.filter-panel__core-picker .filter-panel__subject-list { max-height: 10rem; }

.filter-panel__incompatible {
  display: grid;
  gap: var(--bcsp-space-1);
  padding-top: var(--bcsp-space-1);
}

/* ---- Dictionary combobox / option lists ---- */
.filter-panel__dictionary {
  position: relative;
  display: block;
  min-width: 0;
}

.filter-panel__dictionary > * + * { margin-top: var(--bcsp-space-2); }

.filter-panel__dictionary-options {
  display: none;
  max-height: min(16rem, 42vh);
  padding-right: 0.5rem;
  overflow-x: hidden;
  overflow-y: auto;
  overscroll-behavior: contain;
  touch-action: pan-y;
  scrollbar-width: none;
  scrollbar-gutter: auto;
}

.filter-panel__dictionary-options[data-open='true'] { display: grid; }

/* The searchable combobox pops over the tokens below it (static vertical position = just under the input). */
.filter-panel__dictionary-input + .filter-panel__dictionary-options {
  position: absolute;
  top: auto;
  right: 0;
  left: 0;
  z-index: var(--bcsp-z-popover);
  margin-top: 0.25rem;
  padding: 0.25rem 0.5rem 0.25rem 0.25rem;
  border: 1px solid var(--bcsp-line);
  border-radius: var(--bcsp-radius-3);
  background: var(--bcsp-paper-raised);
  box-shadow: var(--bcsp-elev-2);
}

.filter-panel__dictionary-options > .bcsp-field__helper { padding: 0.5rem 0.625rem; }

.filter-panel__dictionary-option {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 0.625rem;
  min-height: var(--bcsp-control-h);
  align-items: center;
  padding: 0.375rem 0.625rem;
  border-radius: var(--bcsp-radius-2);
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-body);
  line-height: 1.25rem;
  cursor: pointer;
}

/* The glyph span stays for its text; visually it is a CSS checkbox. */
.filter-panel__dictionary-option > span:first-child {
  display: inline-flex;
  width: 1rem;
  height: 1rem;
  align-items: center;
  justify-content: center;
  border: 1.5px solid var(--bcsp-line-strong);
  border-radius: var(--bcsp-radius-1);
  color: transparent;
  background: var(--bcsp-paper-raised);
  font-size: 0;
  line-height: 0;
}

.filter-panel__dictionary-option > span:last-child { overflow-wrap: anywhere; }

.filter-panel__dictionary-option[aria-selected='true'] {
  color: var(--bcsp-ink);
  background: var(--bcsp-accent-tint);
}

.filter-panel__dictionary-option[aria-selected='true'] > span:first-child {
  border-color: var(--bcsp-accent);
  background: var(--bcsp-accent);
}

.filter-panel__dictionary-option[aria-selected='true'] > span:first-child::after {
  width: 0.25rem;
  height: 0.5rem;
  border-right: 2px solid var(--bcsp-accent-ink);
  border-bottom: 2px solid var(--bcsp-accent-ink);
  transform: translateY(-1px) rotate(45deg);
  content: '';
}

.filter-panel__dictionary-option[data-active='true'] {
  outline: 2px solid var(--bcsp-focus);
  outline-offset: -2px;
}

/* ---- Hidden-until-needed scrollbars (spec 11.2) ---- */
.filter-panel__subject-list::-webkit-scrollbar,
.filter-panel__dictionary-options::-webkit-scrollbar,
.filter-panel__chips::-webkit-scrollbar {
  width: 0;
  height: 0;
}

.filter-panel__subject-list:hover,
.filter-panel__subject-list:focus-within,
.filter-panel__dictionary-options:hover,
.filter-panel__dictionary-options:focus-within,
.filter-panel__chips:hover,
.filter-panel__chips:focus-within {
  scrollbar-width: thin;
  scrollbar-color: var(--bcsp-line-strong) transparent;
}

.filter-panel__subject-list:hover::-webkit-scrollbar,
.filter-panel__subject-list:focus-within::-webkit-scrollbar,
.filter-panel__dictionary-options:hover::-webkit-scrollbar,
.filter-panel__dictionary-options:focus-within::-webkit-scrollbar,
.filter-panel__chips:hover::-webkit-scrollbar,
.filter-panel__chips:focus-within::-webkit-scrollbar { width: 8px; }

.filter-panel__subject-list::-webkit-scrollbar-thumb,
.filter-panel__dictionary-options::-webkit-scrollbar-thumb,
.filter-panel__chips::-webkit-scrollbar-thumb {
  border: 2px solid transparent;
  border-radius: var(--bcsp-radius-pill);
  background: var(--bcsp-line-strong);
  background-clip: padding-box;
}

.filter-panel__subject-list::-webkit-scrollbar-track,
.filter-panel__dictionary-options::-webkit-scrollbar-track,
.filter-panel__chips::-webkit-scrollbar-track { background: transparent; }

/* ---- Credits, availability ---- */
.filter-panel__credit-range {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: var(--bcsp-space-1);
}

.filter-panel__credit-range > label { min-width: 0; }

.filter-panel__availability {
  display: grid;
  gap: var(--bcsp-space-2);
  min-width: 0;
}

.filter-panel__availability-editor {
  display: grid;
  grid-template-columns: 1.2fr 1fr 1fr;
  gap: var(--bcsp-space-1);
  align-items: end;
}

.filter-panel__availability-editor > label {
  display: block;
  min-width: 0;
}

.filter-panel__availability-add {
  grid-column: 1 / -1;
  width: 100%;
}

.filter-panel__window-list {
  display: flex;
  flex-wrap: wrap;
  gap: var(--bcsp-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}

.filter-panel__window-list li {
  display: inline-flex;
  min-height: 2rem;
  align-items: center;
  gap: 0.25rem;
  padding: 0 0.25rem 0 0.75rem;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ink-2);
  background: var(--bcsp-surface-2);
}

.filter-panel__window-list samp {
  font-size: var(--bcsp-text-data);
  font-variant-numeric: tabular-nums;
}

.filter-panel__window-list button {
  position: relative;
  display: inline-flex;
  min-height: 2rem;
  align-items: center;
  padding: 0 0.625rem;
  border: 0;
  border-radius: var(--bcsp-radius-pill);
  color: var(--bcsp-ink-2);
  background: transparent;
  font-family: var(--bcsp-font-sans);
  font-size: var(--bcsp-text-meta);
  font-weight: 600;
  letter-spacing: 0;
  text-transform: none;
  cursor: pointer;
}

/* ---- Footer note ---- */
.filter-panel__footer {
  display: grid;
  padding: var(--bcsp-space-1) var(--bcsp-space-4) var(--bcsp-space-3);
}

.filter-panel__footer-note {
  margin: 0;
  color: var(--bcsp-ink-muted);
  font-size: var(--bcsp-text-meta);
  line-height: var(--bcsp-lh-meta);
}

@media (hover: hover) and (pointer: fine) {
  .filter-panel__check:hover:not(:has(input:disabled)):not(:has(input:checked)) { background: var(--bcsp-surface-2); }
  .filter-panel__checks--pills .filter-panel__check:hover:not(:has(input:disabled)):not(:has(input:checked)) { background: var(--bcsp-surface-3); }
  .filter-panel__chip button:hover:not(:disabled),
  .filter-panel__token button:hover:not(:disabled),
  .filter-panel__window-list button:hover:not(:disabled) {
    color: var(--bcsp-accent-text);
    background: var(--bcsp-accent-tint);
  }
  .filter-panel__minor-action:hover:not(:disabled) {
    border-color: var(--bcsp-ink-muted);
    background: var(--bcsp-surface-2);
  }
  .filter-panel__clear-choice:hover:not(:disabled) {
    border-color: transparent;
    color: var(--bcsp-ink);
  }
  .filter-panel__dictionary-option:hover:not([aria-selected='true']) { background: var(--bcsp-surface-2); }
  .filter-panel__input:hover:not(:disabled):not(:focus-visible),
  .filter-panel__select:hover:not(:disabled):not(:focus-visible) { border-color: var(--bcsp-ink-muted); }
}

/* CJK floor: meta text never drops below 13px. */
[data-bcsp-locale='zh-CN'] .filter-panel__active-title,
[data-bcsp-locale='zh-CN'] .filter-panel__chip,
[data-bcsp-locale='zh-CN'] .filter-panel__token,
[data-bcsp-locale='zh-CN'] .filter-panel__sub-label,
[data-bcsp-locale='zh-CN'] .filter-panel__subject-count,
[data-bcsp-locale='zh-CN'] .filter-panel__validation-error,
[data-bcsp-locale='zh-CN'] .filter-panel__check--explained small,
[data-bcsp-locale='zh-CN'] .filter-panel__incomplete small,
[data-bcsp-locale='zh-CN'] .filter-panel__footer-note,
[data-bcsp-locale='zh-CN'] .filter-panel__group-count,
[data-bcsp-locale='zh-CN'] .filter-panel__active::after {
  font-size: 0.8125rem;
  line-height: 1.25rem;
}

@container (max-width: 20rem) {
  .filter-panel__availability-editor,
  .filter-panel__subject-search { grid-template-columns: minmax(0, 1fr); }
  .filter-panel__subject-count { min-height: 0; text-align: left; }
  .filter-panel__input-action {
    grid-template-columns: minmax(0, 1fr);
    gap: var(--bcsp-space-1);
  }
  .filter-panel__input-action .filter-panel__input,
  .filter-panel__input-action .filter-panel__minor-action {
    margin: 0;
    border-radius: var(--bcsp-radius-2);
  }
}
`;

export function FilterPanel({
  schema,
  discovery,
  value,
  onChange,
  onSubmit,
  formId,
  loadOptions,
  disabled = false,
  searchAvailable = true,
  validationIssue,
}: FilterPanelProps) {
  const i18n = useBcspI18n();
  const formRef = useRef<HTMLFormElement>(null);
  const activeStripRef = useRef<HTMLElement>(null);
  const groupIdBase = useId();
  const [subjectQuery, setSubjectQuery] = useState('');

  // The sticky group heads sit directly under the sticky active strip, whose
  // height depends on how many chips wrap; publish the measured height so the
  // heads and the invalid-row scroll margin stay below it.
  useLayoutEffect(() => {
    const form = formRef.current;
    const strip = activeStripRef.current;
    if (form === null || strip === null || typeof ResizeObserver === 'undefined') return undefined;
    const publish = () => {
      const height = strip.getBoundingClientRect().height;
      if (height > 0) form.style.setProperty('--bcsp-rail-strip-h', `${height}px`);
    };
    publish();
    const observer = new ResizeObserver(publish);
    observer.observe(strip);
    return () => observer.disconnect();
  }, []);

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

  const groupedFields = useMemo(() => {
    const members = FILTER_GROUPS.map(() => [] as FilterFieldSchemaV1[]);
    for (const field of fields) members[groupIndexFor(field.stableId)]?.push(field);
    return FILTER_GROUPS.map((group, index) => ({ group, members: members[index] ?? [] }));
  }, [fields]);

  const invalidField = useMemo(() => {
    if (validationIssue === undefined) return undefined;
    const requestField = VALIDATION_FIELD[validationIssue.issue];
    return requestField === undefined
      ? undefined
      : fields.find((field) => field.requestField === requestField);
  }, [fields, validationIssue]);

  useEffect(() => {
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
  // Keyed by the canonical (request-form, uppercase) code so a persisted `AHO`
  // matches the dictionary's `AHo`; `displayCode` keeps the dictionary spelling.
  const coreDictionary = useMemo(() => {
    const entries = new Map<string, { displayCode: string; descriptions: Set<string>; incomplete: boolean }>();
    for (const dictionary of selectedCoreDictionaries) {
      for (const option of dictionary.options) {
        const canonical = canonicalCoreCode(option.code);
        const entry = entries.get(canonical)
          ?? { displayCode: option.code, descriptions: new Set<string>(), incomplete: false };
        const description = knownText(option.description)?.trim();
        if (description === undefined || description === null || description.length === 0) {
          entry.incomplete = true;
        } else {
          entry.descriptions.add(description);
        }
        entries.set(canonical, entry);
      }
    }
    return [...entries.entries()]
      .sort(([left], [right]) => left.localeCompare(right, 'en-US'))
      .map(([code, entry]) => ({
        code,
        displayCode: entry.displayCode,
        label: !entry.incomplete && entry.descriptions.size === 1
          ? `${entry.displayCode} · ${[...entry.descriptions][0]}`
          : entry.displayCode,
      }));
  }, [selectedCoreDictionaries]);
  const validCoreCodes = useMemo(() => new Set(coreDictionary.map(({ code }) => code)), [coreDictionary]);
  const coreLabels = useMemo<CoreLabels>(
    () => new Map(coreDictionary.map(({ code, label }) => [code, label])),
    [coreDictionary],
  );
  const coreOptions = coreDictionary;
  // One entry per canonical code: the first stored spelling represents every
  // case variant, and removing it drops all of them.
  const incompatibleCoreCodes = useMemo(() => {
    if (coreDictionaryLoading) return [];
    const seen = new Set<string>();
    return value.core.codes.filter((code) => {
      const canonical = canonicalCoreCode(code);
      if (validCoreCodes.has(canonical) || seen.has(canonical)) return false;
      seen.add(canonical);
      return true;
    });
  }, [coreDictionaryLoading, validCoreCodes, value.core.codes]);

  const summaries = fields.map((field) => ({ field, summary: fieldSummary(field, value, i18n, coreLabels) }))
    .filter((entry): entry is { field: FilterFieldSchemaV1; summary: string } => entry.summary !== null);
  const clearable = summaries.filter(({ field }) => field.requestField !== 'term' && field.requestField !== 'campuses');

  const update = <K extends keyof FilterStateV1>(key: K, next: FilterStateV1[K]) => {
    onChange({ ...value, [key]: next });
  };
  const updateIncomplete = (
    key: keyof FilterStateV1['includeIncomplete'],
    checked: boolean,
  ) => update('includeIncomplete', { ...value.includeIncomplete, [key]: checked });
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
      case 'courseNumberBands':
        return <DictionaryPicker disabled={disabled} field="COURSE_NUMBER_BAND" label={label}
          loadOptions={loadOptions} values={value.courseNumberBands.map(String)}
          onChange={(next) => update('courseNumberBands', next
            .map((entry) => Number(entry))
            .filter((entry) => Number.isSafeInteger(entry) && entry >= 0 && entry % 100 === 0)
            .sort((left, right) => left - right))} />;
      case 'levels':
        return <DictionaryPicker disabled={disabled} field="COURSE_LEVEL" label={label}
          loadOptions={loadOptions} values={value.levels} variant="pills"
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
                      {coreOptions.map(({ code, displayCode, label: optionLabel }) => (
                        <label className="filter-panel__check" key={code}>
                          <input
                            type="checkbox"
                            checked={hasCoreCode(value.core.codes, code)}
                            disabled={disabled}
                            onChange={(event) => update('core', {
                              ...value.core,
                              codes: toggleCoreCode(value.core.codes, code, event.target.checked),
                            })}
                            value={displayCode}
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
                            codes: toggleCoreCode(value.core.codes, code, false),
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
        return <PrerequisiteControl
          disabled={disabled}
          includeIncomplete={value.includeIncomplete.prerequisite}
          onIncludeIncompleteChange={(checked) => updateIncomplete('prerequisite', checked)}
          onValueChange={(next) => update('prerequisite', next)}
          value={value.prerequisite}
        />;
      case 'sectionIndexes':
        return <TokenListControl label={i18n.t('filter.section_indexes')} values={value.sectionIndexes}
          onChange={(next) => update('sectionIndexes', next)} disabled={disabled} placeholder={i18n.t('filter.five_digits')} />;
      case 'openStatuses':
        return <CheckboxSet label={label} options={OPEN_STATES} values={value.openStatuses} variant="pills"
          onChange={(next) => update('openStatuses', next)} disabled={disabled} />;
      case 'modalities':
        return <div className="filter-panel__control">
          <CheckboxSet label={label} options={MODALITIES} values={value.modalities} variant="pills"
            onChange={(next) => update('modalities', next)} disabled={disabled} />
          <IncompleteToggle checked={value.includeIncomplete.modality} disabled={disabled}
            onChange={(checked) => updateIncomplete('modality', checked)} />
        </div>;
      case 'synchronicities':
        return <div className="filter-panel__control">
          <p className="bcsp-field__helper">{i18n.t('filter.flt-s04b_help')}</p>
          <CheckboxSet label={label} options={SYNCHRONICITIES} values={value.synchronicities} variant="pills"
            help={SYNCHRONICITY_HELP}
            onChange={(next) => update('synchronicities', next)} disabled={disabled} />
          <IncompleteToggle checked={value.includeIncomplete.synchronicity} disabled={disabled}
            helpKey="filter.flt-s04b_incomplete_help"
            onChange={(checked) => updateIncomplete('synchronicity', checked)} />
        </div>;
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
      case 'includeIncomplete':
        return null;
    }
  };

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (disabled || !searchAvailable || value.term === null || value.campuses.length === 0) return;
    onSubmit();
  };

  const renderRow = (field: FilterFieldSchemaV1) => {
    const index = fields.indexOf(field);
    const invalid = field.stableId === invalidField?.stableId;
    const errorId = `filter-panel-error-${field.stableId}`;
    return (
      <fieldset
        className="filter-panel__row"
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

  return (
    <>
      <style data-bcsp-filter-panel="">{FILTER_PANEL_CSS}</style>
      <form ref={formRef} id={formId} className="filter-panel" aria-label={i18n.t('filter.form_label')} onSubmit={submit}>
        <fieldset
          aria-busy={disabled && searchAvailable ? true : undefined}
          className="filter-panel__gate"
          disabled={disabled || !searchAvailable}
        >
        <legend className="bcsp-visually-hidden">{i18n.t('filter.form_label')}</legend>
        <section
          className="filter-panel__active"
          aria-labelledby="active-filter-title"
          data-count={summaries.length}
          ref={activeStripRef}
        >
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

        <div className="filter-panel__grid" data-filter-fields="03-18">
          {groupedFields.map(({ group, members }) => {
            if (members.length === 0) return null;
            const titleId = `${groupIdBase}-${group.key}`;
            const activeInGroup = summaries
              .filter(({ field }) => members.includes(field)).length;
            return (
              <section
                aria-labelledby={titleId}
                className="filter-panel__group"
                data-count={activeInGroup}
                data-filter-group={group.key}
                key={group.key}
                role="group"
              >
                <div className="filter-panel__group-head">
                  <h4 className="filter-panel__group-title" id={titleId}>{i18n.t(group.titleKey)}</h4>
                  <span
                    aria-label={i18n.t('filter.group.active_count', { count: i18n.formatNumber(activeInGroup) })}
                    className="filter-panel__group-count"
                  >
                    {i18n.formatNumber(activeInGroup)}
                  </span>
                </div>
                <div className="filter-panel__group-body">{members.map(renderRow)}</div>
              </section>
            );
          })}
        </div>

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
