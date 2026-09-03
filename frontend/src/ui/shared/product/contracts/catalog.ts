import type {
  CatalogFieldKnowledge,
  ContractVersionV1,
  CourseGroupKey,
  CourseVariantKey,
  IsoDateTime,
  SectionKey,
  TermCampusKey,
  TraceId,
} from './common';

export type CatalogDiscoverySourceKind = 'SELECTOR' | 'BOOTSTRAP';
export type CatalogDiscoveryAvailability =
  | 'CURRENT'
  | 'STALE_LAST_SUCCESS'
  | 'UNAVAILABLE_NO_FIRST_SUCCESS';
export type CatalogDiscoveryErrorClass =
  | 'TRANSPORT'
  | 'HTTP'
  | 'DECODE'
  | 'VALIDATE'
  | 'PERSIST';

export interface CatalogDiscoveryRequestV1 {
  readonly contractVersion: ContractVersionV1;
}

export interface CatalogDiscoveryPointV1 {
  readonly observationId: TraceId;
  readonly observedAt: IsoDateTime;
  readonly contentVersion: number | null;
}

export interface CatalogDiscoveryErrorV1 {
  readonly class: CatalogDiscoveryErrorClass;
  readonly code: string;
}

export interface CatalogDiscoveryStatusV1 {
  readonly availability: CatalogDiscoveryAvailability;
  readonly latestAttempt: CatalogDiscoveryPointV1 | null;
  readonly lastSuccess: CatalogDiscoveryPointV1 | null;
  readonly isStale: boolean;
  readonly error: CatalogDiscoveryErrorV1 | null;
}

export interface CatalogDiscoveryProvenanceV1 {
  readonly observationId: TraceId;
  readonly sourceId: string;
  readonly sourceKind: CatalogDiscoverySourceKind;
  readonly observedAt: IsoDateTime;
  readonly payloadDigest: string;
}

export interface CatalogProvenanceV1 {
  readonly observationId: TraceId;
  readonly source: 'RUTGERS_CATALOG';
  readonly target: TermCampusKey;
  readonly observedAt: IsoDateTime;
  readonly payloadDigest: string;
}

export type CatalogSubjectProvenanceV1 =
  | {
      readonly kind: 'DISCOVERY';
      readonly discovery: CatalogDiscoveryProvenanceV1;
    }
  | {
      readonly kind: 'CATALOG';
      readonly contentVersion: number;
      readonly catalog: CatalogProvenanceV1;
    };

export interface CatalogDiscoverySourceV1 {
  readonly sourceId: string;
  readonly sourceKind: CatalogDiscoverySourceKind;
  readonly sourceVersion: CatalogFieldKnowledge<string>;
  readonly payloadDigest: string;
  readonly observedAt: IsoDateTime;
}

export interface CatalogTargetV1 {
  readonly key: TermCampusKey;
  readonly termLabel: CatalogFieldKnowledge<string>;
  readonly campusLabel: CatalogFieldKnowledge<string>;
  readonly provenance: CatalogDiscoveryProvenanceV1;
}

export interface CatalogSubjectV1 {
  readonly target: TermCampusKey;
  readonly code: string;
  readonly label: CatalogFieldKnowledge<string>;
  readonly provenance: CatalogSubjectProvenanceV1;
}

export interface CatalogCoreCodeOptionV1 {
  readonly code: string;
  readonly description: CatalogFieldKnowledge<string>;
}

export interface CatalogCoreCodeDictionaryV1 {
  readonly target: TermCampusKey;
  readonly contentVersion: number;
  readonly provenance: CatalogProvenanceV1;
  readonly options: readonly CatalogCoreCodeOptionV1[];
}

export interface CatalogDiscoveryResponseV1 {
  readonly contractVersion: ContractVersionV1;
  readonly observedAt: IsoDateTime;
  readonly status: CatalogDiscoveryStatusV1;
  readonly sources: readonly CatalogDiscoverySourceV1[];
  readonly targets: readonly CatalogTargetV1[];
  readonly subjects: readonly CatalogSubjectV1[];
  readonly coreCodeDictionaries: readonly CatalogCoreCodeDictionaryV1[];
}

export interface CatalogOccurrenceKeyV1 {
  readonly section: SectionKey;
  readonly ordinal: number;
}

export interface NormalizedCourseGroupV1 {
  readonly key: CourseGroupKey;
  readonly variantKeys: readonly CourseVariantKey[];
}

export type CatalogPrerequisiteState = 'HAS' | 'NONE_REPORTED' | 'UNKNOWN';
export type CatalogModality =
  | 'ON_CAMPUS_OR_IN_PERSON'
  | 'HYBRID'
  | 'ONLINE'
  | 'OTHER'
  | 'UNKNOWN'
  | 'UNKNOWN_CONFLICT';
export type CatalogSynchronicity =
  | 'SYNC' | 'ASYNC' | 'MIXED' | 'BY_ARRANGEMENT' | 'UNSPECIFIED' | 'UNKNOWN';

export interface NormalizedCourseVariantV1 {
  readonly key: CourseVariantKey;
  readonly title: CatalogFieldKnowledge<string>;
  readonly expandedTitle: CatalogFieldKnowledge<string>;
  readonly description: CatalogFieldKnowledge<string>;
  readonly notes: CatalogFieldKnowledge<string>;
  readonly subjectGroupNotes: CatalogFieldKnowledge<string>;
  readonly subjectNotes: CatalogFieldKnowledge<string>;
  readonly unitNotes: CatalogFieldKnowledge<string>;
  readonly synopsisUrl: CatalogFieldKnowledge<string>;
  readonly prerequisiteNotes: CatalogFieldKnowledge<string>;
  readonly prerequisiteState: CatalogPrerequisiteState;
  readonly credits: CatalogFieldKnowledge<string>;
  readonly level: CatalogFieldKnowledge<string>;
  readonly subjectCode: CatalogFieldKnowledge<string>;
  readonly subjectDescription: CatalogFieldKnowledge<string>;
  readonly courseNumber: CatalogFieldKnowledge<string>;
  readonly supplementCode: CatalogFieldKnowledge<string>;
  readonly schoolCode: CatalogFieldKnowledge<string>;
  readonly offeringUnit: CatalogFieldKnowledge<string>;
  readonly offeringUnitTitle: CatalogFieldKnowledge<string>;
  readonly coreCodes: CatalogFieldKnowledge<readonly string[]>;
  readonly campusLocations: CatalogFieldKnowledge<readonly string[]>;
  readonly sectionKeys: readonly SectionKey[];
}

export interface CatalogUnitMajorV1 {
  readonly unitCode: string;
  readonly majorCode: string;
}

export interface CatalogCommentV1 {
  readonly code: CatalogFieldKnowledge<string>;
  readonly description: CatalogFieldKnowledge<string>;
}

export interface CatalogSnapshotOpenStatusV1 {
  readonly value: CatalogFieldKnowledge<boolean>;
  readonly provenance: 'CATALOG_SNAPSHOT_ONLY';
}

export interface NormalizedSectionV1 {
  readonly key: SectionKey;
  readonly variantKey: CourseVariantKey;
  readonly sectionNumber: CatalogFieldKnowledge<string>;
  readonly subtitle: CatalogFieldKnowledge<string>;
  readonly subtopic: CatalogFieldKnowledge<string>;
  readonly sectionNotes: CatalogFieldKnowledge<string>;
  readonly sessionDates: CatalogFieldKnowledge<string>;
  readonly sessionDatePrintIndicator: CatalogFieldKnowledge<string>;
  readonly comments: CatalogFieldKnowledge<readonly CatalogCommentV1[]>;
  readonly commentsText: CatalogFieldKnowledge<string>;
  readonly crossListedSectionsText: CatalogFieldKnowledge<string>;
  readonly crossListedSectionType: CatalogFieldKnowledge<string>;
  readonly instructors: CatalogFieldKnowledge<readonly string[]>;
  readonly instructorReliability: 'NAME_ONLY';
  readonly rawSectionCourseType: CatalogFieldKnowledge<string>;
  readonly deliveryModality: CatalogModality;
  readonly synchronicity: CatalogSynchronicity;
  readonly examCode: CatalogFieldKnowledge<string>;
  readonly examCodeText: CatalogFieldKnowledge<string>;
  readonly specialPermissionAddCode: CatalogFieldKnowledge<string>;
  readonly specialPermissionAddDescription: CatalogFieldKnowledge<string>;
  readonly specialPermissionDropCode: CatalogFieldKnowledge<string>;
  readonly specialPermissionDropDescription: CatalogFieldKnowledge<string>;
  readonly majorCodes: CatalogFieldKnowledge<readonly string[]>;
  readonly unitCodes: CatalogFieldKnowledge<readonly string[]>;
  readonly minorCodes: CatalogFieldKnowledge<readonly string[]>;
  readonly honorProgramCodes: CatalogFieldKnowledge<readonly string[]>;
  readonly unitMajors: CatalogFieldKnowledge<readonly CatalogUnitMajorV1[]>;
  readonly eligibilityText: CatalogFieldKnowledge<string>;
  readonly openToText: CatalogFieldKnowledge<string>;
  readonly catalogOpenStatus: CatalogSnapshotOpenStatusV1;
  readonly occurrenceKeys: CatalogFieldKnowledge<readonly CatalogOccurrenceKeyV1[]>;
}

export type CatalogTimeKnowledgeV1 =
  | { readonly knowledge: 'MISSING' | 'EXPLICIT_NULL' | 'EMPTY' | 'PARTIAL' | 'INVALID' }
  | { readonly knowledge: 'KNOWN'; readonly startMinute: number; readonly endMinute: number };

export interface NormalizedOccurrenceV1 {
  readonly key: CatalogOccurrenceKeyV1;
  readonly rawCode: CatalogFieldKnowledge<string>;
  readonly rawDescription: CatalogFieldKnowledge<string>;
  readonly modality: CatalogFieldKnowledge<CatalogModality>;
  readonly synchronicity: CatalogFieldKnowledge<CatalogSynchronicity>;
  readonly rawDay: CatalogFieldKnowledge<string>;
  readonly days: CatalogFieldKnowledge<readonly string[]>;
  readonly rawStartTime: CatalogFieldKnowledge<string>;
  readonly rawEndTime: CatalogFieldKnowledge<string>;
  readonly time: CatalogTimeKnowledgeV1;
  readonly startDate: CatalogFieldKnowledge<string>;
  readonly endDate: CatalogFieldKnowledge<string>;
  readonly campus: CatalogFieldKnowledge<string>;
  readonly campusName: CatalogFieldKnowledge<string>;
  readonly building: CatalogFieldKnowledge<string>;
  readonly room: CatalogFieldKnowledge<string>;
  readonly requiredness: 'REQUIRED' | 'OPTIONAL' | 'UNKNOWN_REQUIREDNESS';
  readonly kind: 'SCHEDULED' | 'BY_ARRANGEMENT' | 'UNSPECIFIED';
  readonly evidence: 'NONE' | 'PHYSICAL' | 'REMOTE';
  readonly normalizationReason: string;
}
