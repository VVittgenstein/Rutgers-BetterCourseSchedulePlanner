import crypto from 'node:crypto';

export interface CourseRecord {
  subject: string;
  subjectDescription: string | null;
  schoolCode: string | null;
  schoolDescription: string | null;
  campusCode: string | null;
  courseNumber: string;
  courseString: string | null;
  title: string;
  expandedTitle: string | null;
  level: string | null;
  creditsMin: number | null;
  creditsMax: number | null;
  creditsDisplay: string | null;
  coreCodes: string[];
  prereqHtml: string | null;
  prereqPlain: string | null;
  synopsisUrl: string | null;
  courseNotes: string | null;
  unitNotes: string | null;
  subjectNotes: string | null;
  campusLocations: string[];
  campusLocationCodes: string[];
  openSectionsCount: number | null;
  hasOpenSections: boolean;
  tags: string[];
  searchVector: string;
  supplementCode: string | null;
}

export interface NormalizedCourse {
  key: string;
  record: CourseRecord;
  sections: NormalizedSection[];
  hash: string;
  raw: Record<string, unknown>;
}

export interface SectionRecord {
  sectionNumber: string;
  indexNumber: string;
  openStatus: string;
  isOpen: boolean;
  instructorsText: string | null;
  sectionNotes: string | null;
  comments: NormalizedComment[];
  eligibilityText: string | null;
  openToText: string | null;
  majors: string[];
  minors: string[];
  honorPrograms: string[];
  sectionCourseType: string | null;
  examCode: string | null;
  examCodeText: string | null;
  specialPermissionAddCode: string | null;
  specialPermissionAddDesc: string | null;
  specialPermissionDropCode: string | null;
  specialPermissionDropDesc: string | null;
  printed: string | null;
  sessionPrintIndicator: string | null;
  subtitle: string | null;
  meetingModeSummary: string | null;
  deliveryMethod: string | null;
  hasMeetings: boolean;
  campusCode: string | null;
  sectionCampusLocations: string[];
  commentsText: string | null;
  instructors: string[];
  sectionEligibility: string | null;
}

export interface NormalizedSection {
  key: string;
  record: SectionRecord;
  meetings: NormalizedMeeting[];
  hash: string;
  raw: Record<string, unknown>;
}

export interface NormalizedMeeting {
  meetingDay: string | null;
  weekMask: number | null;
  startTimeLabel: string | null;
  endTimeLabel: string | null;
  startMinutes: number | null;
  endMinutes: number | null;
  meetingModeCode: string | null;
  meetingModeDesc: string | null;
  campusAbbrev: string | null;
  campusLocationCode: string | null;
  campusLocationDesc: string | null;
  buildingCode: string | null;
  roomNumber: string | null;
  pmCode: string | null;
  baClassHours: string | null;
  onlineOnly: boolean;
  hash: string;
}

export interface NormalizedComment {
  code: string | null;
  description: string | null;
}

export type NormalizedCourseMap = Map<string, NormalizedCourse[]>;

const WEEK_MASK: Record<string, number> = {
  M: 1,
  T: 2,
  W: 4,
  TH: 8,
  F: 16,
  SA: 32,
  SU: 64,
};

export function normalizeCoursePayload(payload: unknown): NormalizedCourseMap {
  const map: NormalizedCourseMap = new Map();
  if (!Array.isArray(payload)) {
    return map;
  }
  for (const entry of payload) {
    if (!entry || typeof entry !== 'object') continue;
    const normalized = normalizeCourse(entry as Record<string, unknown>);
    if (!normalized) continue;
    const list = map.get(normalized.record.subject) ?? [];
    list.push(normalized);
    map.set(normalized.record.subject, list);
  }
  return map;
}

function normalizeCourse(raw: Record<string, unknown>): NormalizedCourse | null {
  const subject = ensureString(raw.subject).toUpperCase();
  const courseNumber = ensureString(raw.courseNumber);
  if (!subject || !courseNumber) {
    return null;
  }
  const title = ensureString(raw.title) || `${subject}-${courseNumber}`;
  const campusLocations = toStringArray(raw.campusLocations, (loc) =>
    ensureString((loc as Record<string, unknown>)?.description ?? loc),
  );
  const campusLocationCodes = toStringArray(raw.campusLocations, (loc) =>
    ensureString((loc as Record<string, unknown>)?.code ?? loc),
  );
  const credits = deriveCredits(raw);
  const tags = toStringArray(raw.tags).sort();
  const coreCodes = toStringArray(raw.coreCodes, (code) =>
    ensureString((code as Record<string, unknown>)?.code ?? code),
  ).sort();
  const synopsisUrl = ensureNullable(raw.synopsisUrl ?? raw.syllabusUrl);
  const sectionEntries = Array.isArray(raw.sections) ? raw.sections : [];
  const sections = sectionEntries
    .map((section) => normalizeSection(section as Record<string, unknown>))
    .filter((section): section is NormalizedSection => section !== null)
    .sort((a, b) => a.record.indexNumber.localeCompare(b.record.indexNumber));

  const record: CourseRecord = {
    subject,
    subjectDescription: ensureNullable(raw.subjectDescription),
    schoolCode: ensureNullable((raw.school as Record<string, unknown>)?.code ?? raw.schoolCode),
    schoolDescription: ensureNullable(
      (raw.school as Record<string, unknown>)?.description ?? raw.schoolDescription,
    ),
    campusCode: ensureNullable(raw.campusCode ?? raw.mainCampus ?? null),
    courseNumber,
    courseString: ensureNullable(raw.courseString),
    title,
    expandedTitle: ensureNullable(raw.expandedTitle),
    level: ensureNullable(raw.level),
    creditsMin: credits.min,
    creditsMax: credits.max,
    creditsDisplay: credits.display,
    coreCodes,
    prereqHtml: ensureNullable(raw.preReqNotes),
    prereqPlain: stripHtml(ensureNullable(raw.preReqNotes)),
    synopsisUrl,
    courseNotes: ensureNullable(raw.courseNotes),
    unitNotes: ensureNullable(raw.unitNotes),
    subjectNotes: ensureNullable(raw.subjectNotes),
    campusLocations,
    campusLocationCodes,
    openSectionsCount: typeof raw.openSections === 'number' ? raw.openSections : null,
    hasOpenSections: Boolean(raw.openSections && (raw.openSections as number) > 0),
    tags,
    searchVector: buildSearchVector(subject, courseNumber, title, raw.expandedTitle),
    supplementCode: ensureNullable(raw.supplementCode),
  };

  const hash = hashPayload({ record });
  return {
    key: `${subject}-${courseNumber}`,
    record,
    sections,
    hash,
    raw,
  };
}

function normalizeSection(raw: Record<string, unknown>): NormalizedSection | null {
  const indexNumber = ensureString(raw.index ?? raw.indexNumber);
  if (!indexNumber) {
    return null;
  }
  const sectionNumber = ensureString(raw.number ?? raw.sectionNumber) || indexNumber;
  const openStatus = ensureString(raw.openStatusText ?? raw.openStatus ?? '').toUpperCase() || 'UNKNOWN';
  const isOpen =
    typeof raw.openStatus === 'boolean' ? raw.openStatus : openStatus === 'OPEN';
  const instructors = toStringArray(raw.instructors, (entry) =>
    ensureString((entry as Record<string, unknown>)?.name ?? entry),
  ).sort();
  const comments: NormalizedComment[] = Array.isArray(raw.comments)
    ? (raw.comments as Record<string, unknown>[]).map((comment) => ({
        code: ensureNullable(comment.code),
        description: ensureNullable(comment.description),
      }))
    : [];
  const majors = toStringArray(raw.majors, (entry) => ensureString((entry as Record<string, unknown>)?.code ?? entry)).sort();
  const minors = toStringArray(raw.minors, (entry) => ensureString((entry as Record<string, unknown>)?.code ?? entry)).sort();
  const honorPrograms = toStringArray(raw.honorPrograms, (entry) =>
    ensureString((entry as Record<string, unknown>)?.code ?? entry),
  ).sort();
  const sectionCampusLocations = toStringArray(raw.sectionCampusLocations, (entry) =>
    ensureString((entry as Record<string, unknown>)?.description ?? entry),
  );
  const meetingsRaw = Array.isArray(raw.meetingTimes) ? raw.meetingTimes : [];
  const meetings = meetingsRaw
    .map((meeting) => normalizeMeeting(meeting as Record<string, unknown>))
    .filter((meeting): meeting is NormalizedMeeting => meeting !== null)
    .sort((a, b) => {
      const dayA = a.meetingDay ?? '';
      const dayB = b.meetingDay ?? '';
      if (dayA === dayB) {
        return (a.startTimeLabel ?? '').localeCompare(b.startTimeLabel ?? '');
      }
      return dayA.localeCompare(dayB);
    });
  const record: SectionRecord = {
    sectionNumber,
    indexNumber,
    openStatus,
    isOpen,
    instructorsText: ensureNullable(raw.instructorsText),
    sectionNotes: ensureNullable(raw.sectionNotes),
    comments,
    eligibilityText: ensureNullable(raw.sectionEligibility),
    openToText: ensureNullable(raw.openToText),
    majors,
    minors,
    honorPrograms,
    sectionCourseType: ensureNullable(raw.sectionCourseType),
    examCode: ensureNullable(raw.examCode),
    examCodeText: ensureNullable(raw.examCodeText),
    specialPermissionAddCode: ensureNullable(raw.specialPermissionAddCode),
    specialPermissionAddDesc: ensureNullable(raw.specialPermissionAddCodeDescription),
    specialPermissionDropCode: ensureNullable(raw.specialPermissionDropCode),
    specialPermissionDropDesc: ensureNullable(raw.specialPermissionDropCodeDescription),
    printed: ensureNullable(raw.printed),
    sessionPrintIndicator: ensureNullable(raw.sessionDatePrintIndicator),
    subtitle: ensureNullable(raw.subtitle),
    meetingModeSummary: buildMeetingSummary(meetings),
    deliveryMethod: deriveDeliveryMethod(meetings),
    hasMeetings: meetings.length > 0,
    campusCode: ensureNullable(raw.campusCode ?? raw.campus),
    sectionCampusLocations,
    commentsText: ensureNullable(raw.commentsText),
    instructors,
    sectionEligibility: ensureNullable(raw.sectionEligibility),
  };
  const hash = hashPayload({ record, meetings });
  return {
    key: indexNumber,
    record,
    meetings,
    hash,
    raw,
  };
}

function normalizeMeeting(raw: Record<string, unknown>): NormalizedMeeting | null {
  const meetingModeDesc = ensureNullable(raw.meetingModeDesc);
  const meetingDay = ensureNullable(raw.meetingDay);
  const startTimeLabel = ensureNullable(raw.startTime);
  const endTimeLabel = ensureNullable(raw.endTime);
  const startTimeMilitary = ensureNullable(raw.startTimeMilitary);
  const endTimeMilitary = ensureNullable(raw.endTimeMilitary);
  const weekMask = computeWeekMask(meetingDay);
  const startMinutes = toMinutes(startTimeMilitary);
  const endMinutes = toMinutes(endTimeMilitary);
  const meetingModeCode = ensureNullable(raw.meetingModeCode);
  const onlineOnly = Boolean(
    (meetingModeCode && meetingModeCode === '90') ||
      (meetingModeDesc && meetingModeDesc.toUpperCase().includes('ONLINE')),
  );
  const summary = {
    meetingDay,
    weekMask,
    startTimeLabel,
    endTimeLabel,
    startMinutes,
    endMinutes,
    meetingModeCode,
    meetingModeDesc,
    campusAbbrev: ensureNullable(raw.campusAbbrev),
    campusLocationCode: ensureNullable(raw.campusLocation),
    campusLocationDesc: ensureNullable(raw.campusName),
    buildingCode: ensureNullable(raw.buildingCode),
    roomNumber: ensureNullable(raw.roomNumber),
    pmCode: ensureNullable(raw.pmCode),
    baClassHours: ensureNullable(raw.baClassHours),
    onlineOnly,
  } satisfies Omit<NormalizedMeeting, 'hash'>;
  return {
    ...summary,
    hash: hashPayload(summary),
  };
}

function buildMeetingSummary(meetings: NormalizedMeeting[]): string | null {
  if (meetings.length === 0) {
    return null;
  }
  const labels = Array.from(new Set(meetings.map((meeting) => meeting.meetingModeDesc ?? ''))).filter(Boolean);
  return labels.length > 0 ? labels.join(', ') : null;
}

function deriveDeliveryMethod(meetings: NormalizedMeeting[]): string | null {
  if (meetings.length === 0) {
    return 'async';
  }
  const onlineCount = meetings.filter((meeting) => meeting.onlineOnly).length;
  if (onlineCount === meetings.length) {
    return 'online';
  }
  if (onlineCount > 0) {
    return 'hybrid';
  }
  return 'in_person';
}

function computeWeekMask(day: string | null): number | null {
  if (!day) {
    return null;
  }
  const upper = day.toUpperCase();
  if (WEEK_MASK[upper]) {
    return WEEK_MASK[upper];
  }
  if (upper.length > 1) {
    let mask = 0;
    let buffer = '';
    for (let i = 0; i < upper.length; i += 1) {
      buffer += upper[i];
      if (WEEK_MASK[buffer]) {
        mask |= WEEK_MASK[buffer];
        buffer = '';
      } else if (buffer.length > 2) {
        buffer = upper[i];
      }
    }
    return mask || null;
  }
  return null;
}

function toMinutes(label: string | null): number | null {
  if (!label || label.length < 3) {
    return null;
  }
  const normalized = label.padStart(4, '0');
  const hours = Number.parseInt(normalized.slice(0, 2), 10);
  const minutes = Number.parseInt(normalized.slice(2), 10);
  if (Number.isNaN(hours) || Number.isNaN(minutes)) {
    return null;
  }
  return hours * 60 + minutes;
}

function deriveCredits(raw: Record<string, unknown>): {
  min: number | null;
  max: number | null;
  display: string | null;
} {
  const creditValue = typeof raw.credits === 'number' ? raw.credits : null;
  const creditObject = raw.creditsObject as Record<string, unknown> | undefined;
  const description = ensureNullable(creditObject?.description) ?? (creditValue ? `${creditValue}` : null);
  if (creditObject && typeof creditObject === 'object') {
    const min = Number.parseFloat(ensureString(creditObject.minimum ?? ''));
    const max = Number.parseFloat(ensureString(creditObject.maximum ?? ''));
    return {
      min: Number.isNaN(min) ? creditValue : min,
      max: Number.isNaN(max) ? creditValue : max,
      display: description,
    };
  }
  return {
    min: creditValue,
    max: creditValue,
    display: description,
  };
}

function buildSearchVector(subject: string, courseNumber: string, title: string, expanded?: unknown): string {
  const expandedTitle = ensureString(expanded);
  return [subject, courseNumber, title, expandedTitle].filter(Boolean).join(' ');
}

export function hashPayload(record: unknown): string {
  return crypto.createHash('sha1').update(stableStringify(record)).digest('hex');
}

export function stableStringify(value: unknown): string {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map((item) => stableStringify(item)).join(',')}]`;
  }
  const entries = Object.entries(value as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b));
  return `{${entries.map(([key, val]) => `${JSON.stringify(key)}:${stableStringify(val)}`).join(',')}}`;
}

function ensureString(value: unknown): string {
  if (typeof value === 'string') {
    return value.trim();
  }
  if (value === undefined || value === null) {
    return '';
  }
  return String(value).trim();
}

function ensureNullable(value: unknown): string | null {
  const normalized = ensureString(value);
  return normalized.length === 0 ? null : normalized;
}

function toStringArray(
  value: unknown,
  extractor: (entry: unknown) => string = ensureString,
): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  const set = new Set<string>();
  for (const entry of value) {
    const extracted = extractor(entry).trim();
    if (extracted) {
      set.add(extracted);
    }
  }
  return Array.from(set.values()).sort();
}

function stripHtml(value: string | null): string | null {
  if (!value) {
    return null;
  }
  const plain = value.replace(/<[^>]*>/g, ' ').replace(/&nbsp;/gi, ' ').replace(/\s+/g, ' ').trim();
  return plain.length === 0 ? null : plain;
}
