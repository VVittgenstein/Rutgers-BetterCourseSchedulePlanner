import type {
  AvailabilityWindowV1,
  CourseQueryRequestV1,
  CourseSortV1,
  CreditRangeV1,
  EligibilityUnitMajorV1,
  FilterFieldId,
  FilterRequestV1,
  FilterValuesV1,
  ModalityFilterV1,
  PageRequestV1,
  SectionQueryRequestV1,
  SectionSortV1,
} from './contracts/query';
import type { CatalogSynchronicity } from './contracts/catalog';

export const FILTER_FIELD_IDS = Object.freeze([
  'FLT-C01', 'FLT-C02', 'FLT-C03', 'FLT-C04', 'FLT-C05',
  'FLT-C06', 'FLT-C07', 'FLT-C08', 'FLT-C09', 'FLT-C10',
  'FLT-S01', 'FLT-S02', 'FLT-S03', 'FLT-S04a', 'FLT-S04b',
  'FLT-S05', 'FLT-S06', 'FLT-S07', 'FLT-S08', 'FLT-S09',
  'FLT-S10', 'FLT-S11',
] as const satisfies readonly FilterFieldId[]);

export const FILTER_REQUEST_FIELDS = Object.freeze([
  'term', 'campuses', 'subjects', 'text', 'courseNumbers', 'levels', 'credits',
  'core', 'prerequisite', 'courseLocations', 'sectionIndexes', 'sectionNumbers',
  'openStatuses', 'modalities', 'synchronicities', 'instructors', 'availability',
  'meetingLocations', 'buildingRoom', 'examCodes', 'permission', 'eligibility',
] as const satisfies readonly (keyof FilterValuesV1)[]);

export interface FilterStateV1 extends Omit<FilterValuesV1, 'term'> {
  readonly term: string | null;
}

export type FilterSerializationIssue =
  | 'TERM_REQUIRED'
  | 'INVALID_TEXT'
  | 'INVALID_TOKEN'
  | 'INVALID_CREDIT_RANGE'
  | 'INVALID_AVAILABILITY'
  | 'INVALID_SECTION_INDEX'
  | 'INVALID_PAGE'
  | 'TOO_MANY_VALUES';

export class FilterSerializationError extends Error {
  readonly issue: FilterSerializationIssue;

  constructor(issue: FilterSerializationIssue, message: string) {
    super(message);
    this.name = 'FilterSerializationError';
    this.issue = issue;
  }
}

export function createNeutralFilterState(term: string | null = null): FilterStateV1 {
  return {
    term,
    campuses: [],
    subjects: [],
    text: null,
    courseNumbers: [],
    levels: [],
    credits: null,
    core: { codes: [], mode: 'ANY' },
    prerequisite: 'ANY',
    courseLocations: [],
    sectionIndexes: [],
    sectionNumbers: [],
    openStatuses: [],
    modalities: [],
    synchronicities: [],
    instructors: [],
    availability: [],
    meetingLocations: [],
    buildingRoom: { buildingCodes: [], roomNumbers: [] },
    examCodes: [],
    permission: 'ANY',
    eligibility: {
      majorCodes: [],
      minorCodes: [],
      honorProgramCodes: [],
      unitCodes: [],
      unitMajors: [],
    },
  };
}

const utf8 = new TextEncoder();

function fail(issue: FilterSerializationIssue, message: string): never {
  throw new FilterSerializationError(issue, message);
}

function canonicalIdentity(value: string, field: string): string {
  if (value.length === 0 || value.trim() !== value || /[\p{Cc}]/u.test(value)) {
    return fail('INVALID_TOKEN', `${field} is not a canonical identity`);
  }
  return value;
}

function normalizeToken(value: string, field: string, uppercase: boolean): string {
  const normalized = value.trim();
  if (normalized.length === 0 || utf8.encode(normalized).length > 256 || /[\p{Cc}]/u.test(normalized)) {
    return fail('INVALID_TOKEN', `${field} contains an invalid token`);
  }
  return uppercase ? normalized.replace(/[a-z]/gu, (character) => character.toUpperCase()) : normalized;
}

function compareUtf8(left: string, right: string): number {
  const leftBytes = utf8.encode(left);
  const rightBytes = utf8.encode(right);
  const length = Math.min(leftBytes.length, rightBytes.length);
  for (let index = 0; index < length; index += 1) {
    const difference = (leftBytes[index] ?? 0) - (rightBytes[index] ?? 0);
    if (difference !== 0) return difference;
  }
  return leftBytes.length - rightBytes.length;
}

function canonicalStrings(
  values: readonly string[],
  field: string,
  normalize: (value: string, field: string) => string,
): string[] {
  if (values.length > 100) fail('TOO_MANY_VALUES', `${field} exceeds 100 values`);
  return [...new Set(values.map((value) => normalize(value, field)))].sort(compareUtf8);
}

function normalizeText(value: string | null): string | null {
  if (value === null || value.trim().length === 0) return null;
  if ([...value].some((character) => /\p{Cc}/u.test(character) && !/\s/u.test(character))) {
    fail('INVALID_TEXT', 'text contains a control character');
  }
  const tokens = value.trim().split(/\s+/u);
  const normalized = tokens.join(' ');
  if (
    tokens.length > 32
    || utf8.encode(normalized).length > 512
    || tokens.some((token) => utf8.encode(token).length > 128 || !/[\p{L}\p{N}]/u.test(token))
  ) {
    fail('INVALID_TEXT', 'text does not satisfy the v1 token contract');
  }
  return normalized;
}

function normalizeCredits(value: CreditRangeV1 | null): CreditRangeV1 | null {
  if (value === null) return null;
  const { minimumHundredths, maximumHundredths } = value;
  if (
    (minimumHundredths === null && maximumHundredths === null)
    || [minimumHundredths, maximumHundredths].some(
      (bound) => bound !== null && (!Number.isSafeInteger(bound) || bound < 0),
    )
    || (minimumHundredths !== null && maximumHundredths !== null && minimumHundredths > maximumHundredths)
  ) {
    fail('INVALID_CREDIT_RANGE', 'credits must be a nonempty ordered range');
  }
  return { minimumHundredths, maximumHundredths };
}

function normalizeAvailability(values: readonly AvailabilityWindowV1[]): AvailabilityWindowV1[] {
  if (values.length > 64) fail('TOO_MANY_VALUES', 'availability exceeds 64 windows');
  const normalized = values.map((value) => {
    if (
      !Number.isInteger(value.startMinute)
      || !Number.isInteger(value.endMinute)
      || value.startMinute < 0
      || value.startMinute >= value.endMinute
      || value.endMinute > 1_440
    ) {
      fail('INVALID_AVAILABILITY', 'availability must be an ordered minute range');
    }
    return { ...value };
  });
  const byIdentity = new Map(normalized.map((value) => [
    `${value.weekday}:${String(value.startMinute).padStart(4, '0')}:${String(value.endMinute).padStart(4, '0')}`,
    value,
  ]));
  const weekdayOrder = new Map([
    'MONDAY', 'TUESDAY', 'WEDNESDAY', 'THURSDAY', 'FRIDAY', 'SATURDAY', 'SUNDAY',
  ].map((weekday, index) => [weekday, index]));
  return [...byIdentity.values()].sort((left, right) =>
    (weekdayOrder.get(left.weekday) ?? 0) - (weekdayOrder.get(right.weekday) ?? 0)
    || left.startMinute - right.startMinute
    || left.endMinute - right.endMinute);
}

function normalizeUnitMajors(values: readonly EligibilityUnitMajorV1[]): EligibilityUnitMajorV1[] {
  if (values.length > 100) fail('TOO_MANY_VALUES', 'unitMajors exceeds 100 values');
  const byIdentity = new Map(values.map((value) => {
    const unitCode = normalizeToken(value.unitCode, 'unitMajors.unitCode', true);
    const majorCode = normalizeToken(value.majorCode, 'unitMajors.majorCode', true);
    return [`${unitCode}\u0000${majorCode}`, { unitCode, majorCode }] as const;
  }));
  return [...byIdentity.entries()].sort(([left], [right]) => compareUtf8(left, right)).map(([, value]) => value);
}

export function toFilterValuesV1(state: FilterStateV1): FilterValuesV1 {
  if (state.term === null) fail('TERM_REQUIRED', 'term is required');
  const term = canonicalIdentity(state.term, 'term');
  const sectionIndexes = canonicalStrings(state.sectionIndexes, 'sectionIndexes', (value, field) => {
    const normalized = canonicalIdentity(value, field);
    if (!/^\d{5}$/u.test(normalized)) fail('INVALID_SECTION_INDEX', 'section index must contain five digits');
    return normalized;
  });
  const instructors = canonicalStrings(state.instructors, 'instructors', (value, field) =>
    normalizeToken(value, field, false).split(/\s+/u).join(' '));
  const values: FilterValuesV1 = {
    term,
    campuses: canonicalStrings(state.campuses, 'campuses', canonicalIdentity),
    subjects: canonicalStrings(state.subjects, 'subjects', canonicalIdentity),
    text: normalizeText(state.text),
    courseNumbers: canonicalStrings(state.courseNumbers, 'courseNumbers', (value, field) => normalizeToken(value, field, true)),
    levels: canonicalStrings(state.levels, 'levels', (value, field) => normalizeToken(value, field, true)),
    credits: normalizeCredits(state.credits),
    core: {
      codes: canonicalStrings(state.core.codes, 'core.codes', (value, field) => normalizeToken(value, field, true)),
      mode: state.core.mode,
    },
    prerequisite: state.prerequisite,
    courseLocations: canonicalStrings(state.courseLocations, 'courseLocations', (value, field) => normalizeToken(value, field, true)),
    sectionIndexes,
    sectionNumbers: canonicalStrings(state.sectionNumbers, 'sectionNumbers', (value, field) => normalizeToken(value, field, true)),
    openStatuses: [...new Set(state.openStatuses)].sort((left, right) =>
      ['OPEN', 'CLOSED', 'UNKNOWN'].indexOf(left) - ['OPEN', 'CLOSED', 'UNKNOWN'].indexOf(right)),
    modalities: [...new Set<ModalityFilterV1>(state.modalities)].sort((left, right) =>
      ['ON_CAMPUS_OR_IN_PERSON', 'ONLINE', 'HYBRID', 'OTHER', 'UNKNOWN'].indexOf(left)
      - ['ON_CAMPUS_OR_IN_PERSON', 'ONLINE', 'HYBRID', 'OTHER', 'UNKNOWN'].indexOf(right)),
    synchronicities: [...new Set<CatalogSynchronicity>(state.synchronicities)].sort(compareUtf8),
    instructors,
    availability: normalizeAvailability(state.availability),
    meetingLocations: canonicalStrings(state.meetingLocations, 'meetingLocations', (value, field) => normalizeToken(value, field, true)),
    buildingRoom: {
      buildingCodes: canonicalStrings(state.buildingRoom.buildingCodes, 'buildingRoom.buildingCodes', (value, field) => normalizeToken(value, field, true)),
      roomNumbers: canonicalStrings(state.buildingRoom.roomNumbers, 'buildingRoom.roomNumbers', (value, field) => normalizeToken(value, field, true)),
    },
    examCodes: canonicalStrings(state.examCodes, 'examCodes', (value, field) => normalizeToken(value, field, true)),
    permission: state.permission,
    eligibility: {
      majorCodes: canonicalStrings(state.eligibility.majorCodes, 'eligibility.majorCodes', (value, field) => normalizeToken(value, field, true)),
      minorCodes: canonicalStrings(state.eligibility.minorCodes, 'eligibility.minorCodes', (value, field) => normalizeToken(value, field, true)),
      honorProgramCodes: canonicalStrings(state.eligibility.honorProgramCodes, 'eligibility.honorProgramCodes', (value, field) => normalizeToken(value, field, true)),
      unitCodes: canonicalStrings(state.eligibility.unitCodes, 'eligibility.unitCodes', (value, field) => normalizeToken(value, field, true)),
      unitMajors: normalizeUnitMajors(state.eligibility.unitMajors),
    },
  };
  const total = [
    values.campuses, values.subjects, values.courseNumbers, values.levels, values.core.codes,
    values.courseLocations, values.sectionIndexes, values.sectionNumbers, values.openStatuses,
    values.modalities, values.synchronicities, values.instructors, values.availability,
    values.meetingLocations, values.buildingRoom.buildingCodes, values.buildingRoom.roomNumbers,
    values.examCodes, values.eligibility.majorCodes, values.eligibility.minorCodes,
    values.eligibility.honorProgramCodes, values.eligibility.unitCodes, values.eligibility.unitMajors,
  ].reduce((count, field) => count + field.length, 0);
  if (total > 512) fail('TOO_MANY_VALUES', 'filter request exceeds 512 values');
  return values;
}

export function toFilterRequestV1(state: FilterStateV1): FilterRequestV1 {
  return { contractVersion: 1, values: toFilterValuesV1(state) };
}

export function serializeFilterRequestV1(state: FilterStateV1): string {
  return JSON.stringify(toFilterRequestV1(state));
}

export function createCourseQueryRequestV1(
  state: FilterStateV1,
  page: PageRequestV1 = { page: 1, pageSize: 25 },
  sort: CourseSortV1 = { field: 'RELEVANCE', direction: 'DESCENDING' },
): CourseQueryRequestV1 {
  validatePage(page);
  return { filters: toFilterRequestV1(state), page: { ...page }, sort: { ...sort } };
}

export function createSectionQueryRequestV1(
  state: FilterStateV1,
  page: PageRequestV1 = { page: 1, pageSize: 25 },
  sort: SectionSortV1 = { field: 'SECTION_INDEX', direction: 'ASCENDING' },
): SectionQueryRequestV1 {
  validatePage(page);
  return { filters: toFilterRequestV1(state), page: { ...page }, sort: { ...sort } };
}

function validatePage(page: PageRequestV1): void {
  if (!Number.isInteger(page.page) || !Number.isInteger(page.pageSize) || page.page < 1 || page.pageSize < 1 || page.pageSize > 200) {
    fail('INVALID_PAGE', 'page must be positive and pageSize must not exceed 200');
  }
}
