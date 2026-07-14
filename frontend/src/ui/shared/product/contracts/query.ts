import type {
  CatalogSynchronicity,
  NormalizedCourseGroupV1,
  NormalizedCourseVariantV1,
  NormalizedOccurrenceV1,
  NormalizedSectionV1,
} from './catalog';
import type {
  ContractVersionV1,
  CourseGroupKey,
  IsoDateTime,
  MatchExplanation,
  MatchReasonCode,
  SectionKey,
} from './common';

export type FilterFieldId =
  | 'FLT-C01' | 'FLT-C02' | 'FLT-C03' | 'FLT-C04' | 'FLT-C05'
  | 'FLT-C06' | 'FLT-C07' | 'FLT-C08' | 'FLT-C09' | 'FLT-C10'
  | 'FLT-S01' | 'FLT-S02' | 'FLT-S03' | 'FLT-S04a' | 'FLT-S04b'
  | 'FLT-S05' | 'FLT-S06' | 'FLT-S07' | 'FLT-S08' | 'FLT-S09'
  | 'FLT-S10' | 'FLT-S11';

export type FilterSetModeV1 = 'ANY' | 'ALL';
export type PrerequisiteFilterV1 = 'ANY' | 'HAS' | 'NONE_REPORTED';
export type LiveOpenStateV1 = 'OPEN' | 'CLOSED' | 'UNKNOWN';
export type ModalityFilterV1 =
  | 'ON_CAMPUS_OR_IN_PERSON'
  | 'ONLINE'
  | 'HYBRID'
  | 'OTHER'
  | 'UNKNOWN';
export type PermissionFilterV1 = 'ANY' | 'REQUIRED' | 'NOT_REQUIRED';
export type WeekdayV1 =
  | 'MONDAY' | 'TUESDAY' | 'WEDNESDAY' | 'THURSDAY'
  | 'FRIDAY' | 'SATURDAY' | 'SUNDAY';

export type FilterValueKindV1 =
  | 'TERM_ID' | 'CAMPUS_CODE_SET' | 'SUBJECT_CODE_SET' | 'TEXT_QUERY'
  | 'COURSE_NUMBER_SET' | 'LEVEL_SET' | 'CREDIT_RANGE' | 'CORE_CODE_SET'
  | 'PREREQUISITE_PRESENCE' | 'COURSE_LOCATION_SET' | 'SECTION_INDEX_SET'
  | 'SECTION_NUMBER_SET' | 'OPEN_STATUS_SET' | 'MODALITY_SET'
  | 'SYNCHRONICITY_SET' | 'INSTRUCTOR_NAME_SET' | 'AVAILABILITY_WINDOWS'
  | 'MEETING_LOCATION_SET' | 'BUILDING_ROOM' | 'EXAM_CODE_SET'
  | 'PERMISSION_REQUIREMENT' | 'ELIGIBILITY';
export type FilterSchemaValueV1 =
  | 'REQUIRED' | 'EMPTY_SET' | 'EMPTY_TEXT' | 'UNBOUNDED_RANGE'
  | 'ANY' | 'EMPTY_WINDOWS' | 'EMPTY_COMPOSITE';
export type FilterNormalizationV1 =
  | 'CANONICAL_IDENTITY' | 'TRIM' | 'TRIM_AND_COLLAPSE_WHITESPACE'
  | 'ASCII_UPPERCASE' | 'SORT_DEDUPLICATE' | 'CREDIT_HUNDREDTHS'
  | 'MINUTE_OF_DAY' | 'TOKEN_AND';
export type FilterValidationV1 =
  | 'REQUIRED' | 'DYNAMIC_DICTIONARY' | 'NONEMPTY_WHEN_ACTIVE'
  | 'ORDERED_INCLUSIVE_RANGE' | 'ORDERED_MINUTE_INTERVAL'
  | 'SECTION_INDEX_IDENTITY' | 'STRUCTURED_ONLY' | 'MAX32_TEXT_TOKENS'
  | 'MAX128_TOKEN_BYTES' | 'TOKEN_CONTAINS_ALPHANUMERIC';
export type FilterQueryEncodingV1 =
  | 'EXACT_ONE' | 'EXACT_ANY' | 'TEXT_TOKEN_AND_EXACT_IDENTIFIER_PRIORITY'
  | 'INCLUSIVE_RANGE' | 'EXPLICIT_ANY_ALL' | 'TERNARY_PRESENCE'
  | 'SAME_SECTION_EXACT_ANY' | 'SAME_SECTION_AVAILABILITY_ALL'
  | 'SAME_SECTION_STRUCTURED_DIMENSIONS';
export type FilterCanonicalNeutralV1 =
  | { readonly kind: 'REQUIRED' }
  | { readonly kind: 'JSON'; readonly value: unknown };

export interface FilterFieldSchemaV1 {
  readonly stableId: FilterFieldId;
  readonly requestField: keyof FilterValuesV1;
  readonly scope: 'COURSE' | 'SECTION';
  readonly valueKind: FilterValueKindV1;
  readonly neutral: FilterSchemaValueV1;
  readonly default: FilterSchemaValueV1;
  readonly canonicalNeutral: FilterCanonicalNeutralV1;
  readonly normalization: readonly FilterNormalizationV1[];
  readonly validation: readonly FilterValidationV1[];
  readonly queryEncoding: FilterQueryEncodingV1;
  readonly i18nKey: string;
  readonly chipOrder: number;
}

export interface FilterSchemaV1 {
  readonly contractVersion: ContractVersionV1;
  readonly fields: readonly FilterFieldSchemaV1[];
}

export interface CreditRangeV1 {
  readonly minimumHundredths: number | null;
  readonly maximumHundredths: number | null;
}

export interface AvailabilityWindowV1 {
  readonly weekday: WeekdayV1;
  readonly startMinute: number;
  readonly endMinute: number;
}

export interface CoreFilterV1 {
  readonly codes: readonly string[];
  readonly mode: FilterSetModeV1;
}

export interface BuildingRoomFilterV1 {
  readonly buildingCodes: readonly string[];
  readonly roomNumbers: readonly string[];
}

export interface EligibilityUnitMajorV1 {
  readonly unitCode: string;
  readonly majorCode: string;
}

export interface EligibilityFilterV1 {
  readonly majorCodes: readonly string[];
  readonly minorCodes: readonly string[];
  readonly honorProgramCodes: readonly string[];
  readonly unitCodes: readonly string[];
  readonly unitMajors: readonly EligibilityUnitMajorV1[];
}

export interface FilterValuesV1 {
  readonly term: string;
  readonly campuses: readonly string[];
  readonly subjects: readonly string[];
  readonly text: string | null;
  readonly courseNumbers: readonly string[];
  readonly levels: readonly string[];
  readonly credits: CreditRangeV1 | null;
  readonly core: CoreFilterV1;
  readonly prerequisite: PrerequisiteFilterV1;
  readonly courseLocations: readonly string[];
  readonly sectionIndexes: readonly string[];
  readonly sectionNumbers: readonly string[];
  readonly openStatuses: readonly LiveOpenStateV1[];
  readonly modalities: readonly ModalityFilterV1[];
  readonly synchronicities: readonly CatalogSynchronicity[];
  readonly instructors: readonly string[];
  readonly availability: readonly AvailabilityWindowV1[];
  readonly meetingLocations: readonly string[];
  readonly buildingRoom: BuildingRoomFilterV1;
  readonly examCodes: readonly string[];
  readonly permission: PermissionFilterV1;
  readonly eligibility: EligibilityFilterV1;
}

export interface FilterRequestV1 {
  readonly contractVersion: ContractVersionV1;
  readonly values: FilterValuesV1;
}

export interface PageRequestV1 {
  readonly page: number;
  readonly pageSize: number;
}

export type SortDirectionV1 = 'ASCENDING' | 'DESCENDING';
export type CourseSortFieldV1 = 'RELEVANCE' | 'COURSE_IDENTIFIER' | 'TITLE';
export type SectionSortFieldV1 =
  | 'SECTION_INDEX'
  | 'SECTION_NUMBER'
  | 'COURSE_IDENTIFIER'
  | 'OPEN_STATUS';

export interface CourseSortV1 {
  readonly field: CourseSortFieldV1;
  readonly direction: SortDirectionV1;
}

export interface SectionSortV1 {
  readonly field: SectionSortFieldV1;
  readonly direction: SortDirectionV1;
}

export interface CourseQueryRequestV1 {
  readonly filters: FilterRequestV1;
  readonly page: PageRequestV1;
  readonly sort: CourseSortV1;
}

export interface SectionQueryRequestV1 {
  readonly filters: FilterRequestV1;
  readonly page: PageRequestV1;
  readonly sort: SectionSortV1;
}

export interface CourseDetailRequestV1 {
  readonly contractVersion: ContractVersionV1;
  readonly key: CourseGroupKey;
}

export interface SectionDetailRequestV1 {
  readonly contractVersion: ContractVersionV1;
  readonly key: SectionKey;
}

export interface LiveOpenEvidenceV1 {
  readonly state: LiveOpenStateV1;
  readonly observedAt: IsoDateTime | null;
  readonly freshUntil: IsoDateTime | null;
  readonly uncertainty: MatchReasonCode | null;
}

export interface FilterMatchV1 {
  readonly fieldId: FilterFieldId;
  readonly explanation: MatchExplanation;
}

export interface TextMatchEvidenceV1 {
  readonly exactCourseIdentifier: boolean;
  readonly matchedTokens: readonly string[];
}

export interface SectionQueryItemV1 {
  readonly section: NormalizedSectionV1;
  readonly occurrences: readonly NormalizedOccurrenceV1[];
  readonly open: LiveOpenEvidenceV1;
  readonly explanation: MatchExplanation;
  readonly filterMatches: readonly FilterMatchV1[];
}

export interface SectionSearchItemV1 {
  readonly variant: NormalizedCourseVariantV1;
  readonly section: SectionQueryItemV1;
  readonly courseFilterMatches: readonly FilterMatchV1[];
  readonly textMatch: TextMatchEvidenceV1 | null;
}

export interface CourseVariantQueryItemV1 {
  readonly variant: NormalizedCourseVariantV1;
  readonly explanation: MatchExplanation;
  readonly filterMatches: readonly FilterMatchV1[];
  readonly textMatch: TextMatchEvidenceV1 | null;
  readonly sections: readonly SectionQueryItemV1[];
}

export interface CourseQueryItemV1 {
  readonly group: NormalizedCourseGroupV1;
  readonly explanation: MatchExplanation;
  readonly variants: readonly CourseVariantQueryItemV1[];
}

export interface PageInfoV1 extends PageRequestV1 {
  readonly total: number;
  readonly totalPages: number;
}

export interface CourseQueryResponseV1 {
  readonly contractVersion: ContractVersionV1;
  readonly page: PageInfoV1;
  readonly items: readonly CourseQueryItemV1[];
}

export interface SectionQueryResponseV1 {
  readonly contractVersion: ContractVersionV1;
  readonly page: PageInfoV1;
  readonly items: readonly SectionSearchItemV1[];
}

export interface CourseDetailResponseV1 {
  readonly contractVersion: ContractVersionV1;
  readonly course: CourseQueryItemV1;
}

export interface SectionDetailResponseV1 {
  readonly contractVersion: ContractVersionV1;
  readonly variant: NormalizedCourseVariantV1;
  readonly section: SectionQueryItemV1;
}
