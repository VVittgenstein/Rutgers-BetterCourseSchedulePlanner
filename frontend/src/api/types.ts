export interface CourseSearchResponse {
  meta: {
    page: number;
    pageSize: number;
    total: number;
    hasNext: boolean;
    generatedAt: string;
    version: string;
  };
  data: CourseSearchRow[];
}

export interface CourseSearchRow {
  courseId: number;
  termId: string;
  campusCode: string;
  subjectCode: string;
  courseNumber: string;
  courseString: string | null;
  title: string;
  expandedTitle: string | null;
  level: string | null;
  creditsMin: number | null;
  creditsMax: number | null;
  creditsDisplay: string | null;
  coreAttributes: unknown;
  hasOpenSections: boolean;
  sectionsOpen: number;
  updatedAt: string | null;
  prerequisites: string | null;
  sections?: CourseSectionRow[];
  subject?: {
    code: string;
    description: string | null;
    schoolCode: string | null;
    schoolDescription: string | null;
  };
  sectionsSummary?: {
    total: number;
    open: number;
    deliveryMethods: string[];
  };
}

export interface CourseSectionMeeting {
  meetingDay: string | null;
  startMinutes: number | null;
  endMinutes: number | null;
  campus?: string | null;
  building?: string | null;
  room?: string | null;
}

export interface CourseSectionRow {
  sectionId: number;
  indexNumber: string;
  sectionNumber: string | null;
  openStatus: string | null;
  isOpen: boolean;
  deliveryMethod: string | null;
  campusCode: string | null;
  meetingCampus?: string | null;
  instructorsText?: string | null;
  meetingModeSummary?: string | null;
  meetings: CourseSectionMeeting[];
}

export interface FiltersResponse {
  meta: {
    generatedAt: string;
    version: string;
  };
  data: FiltersPayload;
}

export interface FiltersPayload {
  terms: Array<{
    id: string;
    display: string;
    active?: boolean;
  }>;
  campuses: Array<{
    code: string;
    display: string;
    region?: string;
  }>;
  campusLocations?: Array<{
    code: string;
    description: string;
    campus?: string | null;
  }>;
  subjects: Array<{
    code: string;
    description: string;
    school?: string;
    campus?: string;
  }>;
  coreCodes: Array<{
    code: string;
    description: string;
  }>;
  examCodes?: Array<{
    code: string;
    description?: string | null;
  }>;
  levels: string[];
  deliveryMethods: string[];
  instructors?: Array<{ id: string; name: string }>;
}

export type FetchJobStatus = 'running' | 'success' | 'error';

export interface FetchJob {
  id: string;
  term: string;
  campus: string;
  mode: 'full-init' | 'incremental';
  status: FetchJobStatus;
  startedAt: string;
  finishedAt: string | null;
  message?: string;
  logFile?: string;
}

export interface FetchStatusResponse {
  meta: {
    generatedAt: string;
    version: string;
  };
  data: {
    job: FetchJob | null;
  };
}

export type SubscriptionContactType = 'email' | 'discord_user' | 'discord_channel';

export type SubscriptionPreferencesInput = Partial<{
  notifyOn: Array<'open' | 'waitlist'>;
  maxNotifications: number;
  deliveryWindow: { startMinutes: number; endMinutes: number };
  snoozeUntil: string | null;
  channelMetadata: Record<string, unknown>;
}>;

export interface SubscriptionPreferences {
  notifyOn: Array<'open' | 'waitlist'>;
  maxNotifications: number;
  deliveryWindow: { startMinutes: number; endMinutes: number };
  snoozeUntil: string | null;
  channelMetadata: Record<string, unknown>;
}

export interface SubscribeRequestPayload {
  term: string;
  campus: string;
  sectionIndex: string;
  contactType: SubscriptionContactType;
  contactValue: string;
  locale?: string;
  preferences?: SubscriptionPreferencesInput;
  clientContext?: { ip?: string; userAgent?: string };
  discord?: { guildId?: string; channelId?: string };
}

export interface SubscribeResponsePayload {
  subscriptionId: number;
  status: string;
  requiresVerification: boolean;
  existing: boolean;
  unsubscribeToken: string | null;
  term: string;
  campus: string;
  sectionIndex: string;
  sectionResolved: boolean;
  preferences: SubscriptionPreferences;
  traceId: string;
}

export interface UnsubscribeRequestPayload {
  subscriptionId?: number;
  unsubscribeToken?: string;
  contactValue?: string;
  reason?: string;
}

export interface UnsubscribeResponsePayload {
  subscriptionId: number;
  status: 'unsubscribed';
  previousStatus: string;
  traceId: string;
}
