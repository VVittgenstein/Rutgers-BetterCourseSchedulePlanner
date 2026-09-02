import { describe, expect, it } from 'vitest';

import {
  createNeutralFilterState,
  type CourseQueryRequestV1,
  type FilterStateV1,
} from '../src/ui/shared/product';
import {
  MAX_DIAGNOSIS_PROBES,
  activeRelaxations,
  probeRequest,
  relaxFilters,
  relaxValues,
  type Relaxation,
} from '../src/ui/shared/search';

const TERM = '72026';

/** The P1 filter set: every relaxation candidate except sectionIndexes,
 * instructors, availability, meetingLocations, examCodes, permission, credits,
 * keywords, and subjects. */
const P1_FILTERS: FilterStateV1 = {
  ...createNeutralFilterState(TERM),
  campuses: ['NB'],
  courseNumberBands: [100, 200],
  levels: ['U'],
  core: { codes: ['QR'], mode: 'ANY' },
  prerequisite: 'NONE_REPORTED',
  openStatuses: ['CLOSED'],
  modalities: ['ONLINE'],
  synchronicities: ['ASYNC'],
};

function describeRelaxation(relaxation: Relaxation): string {
  return relaxation.kind === 'CLEAR_FIELD'
    ? `CLEAR_FIELD:${relaxation.field}`
    : `INCLUDE_INCOMPLETE:${relaxation.toggle}`;
}

describe('empty-result diagnosis relaxations', () => {
  it('returns no relaxation for neutral values', () => {
    expect(activeRelaxations({ ...createNeutralFilterState(TERM), campuses: ['NB'] })).toEqual([]);
  });

  it('orders the P1 candidates by priority and caps them at MAX_DIAGNOSIS_PROBES', () => {
    const relaxations = activeRelaxations(P1_FILTERS);
    expect(MAX_DIAGNOSIS_PROBES).toBe(8);
    expect(relaxations).toHaveLength(8);
    expect(relaxations.map(describeRelaxation)).toEqual([
      'INCLUDE_INCOMPLETE:synchronicity',
      'INCLUDE_INCOMPLETE:modality',
      'INCLUDE_INCOMPLETE:prerequisite',
      'CLEAR_FIELD:synchronicities',
      'CLEAR_FIELD:modalities',
      'CLEAR_FIELD:openStatuses',
      'CLEAR_FIELD:prerequisite',
      'CLEAR_FIELD:core',
    ]);
    expect(relaxations.map(({ stableId }) => stableId)).toEqual([
      'FLT-S04b', 'FLT-S04a', 'FLT-C09', 'FLT-S04b', 'FLT-S04a', 'FLT-S03', 'FLT-C09', 'FLT-C08',
    ]);
  });

  it('lists ten candidates for the P1 set before the cap, dropping the lowest priority ones', () => {
    // Remove three high-priority candidates so the tail becomes visible.
    const relaxations = activeRelaxations({
      ...P1_FILTERS,
      includeIncomplete: { prerequisite: true, modality: true, synchronicity: true },
    });
    expect(relaxations.map(describeRelaxation)).toEqual([
      'CLEAR_FIELD:synchronicities',
      'CLEAR_FIELD:modalities',
      'CLEAR_FIELD:openStatuses',
      'CLEAR_FIELD:prerequisite',
      'CLEAR_FIELD:core',
      'CLEAR_FIELD:levels',
      'CLEAR_FIELD:courseNumberBands',
    ]);
  });

  it('offers INCLUDE_INCOMPLETE only while the matching filter is active and the toggle is off', () => {
    const withModalityIncluded = activeRelaxations({
      ...P1_FILTERS,
      includeIncomplete: { prerequisite: false, modality: true, synchronicity: false },
    });
    expect(withModalityIncluded.map(describeRelaxation)).not.toContain('INCLUDE_INCOMPLETE:modality');
    expect(withModalityIncluded.map(describeRelaxation)).toContain('INCLUDE_INCOMPLETE:synchronicity');

    const withoutPrerequisite = activeRelaxations({ ...P1_FILTERS, prerequisite: 'ANY' });
    expect(withoutPrerequisite.map(describeRelaxation)).not.toContain('INCLUDE_INCOMPLETE:prerequisite');
    expect(withoutPrerequisite.map(describeRelaxation)).not.toContain('CLEAR_FIELD:prerequisite');
  });

  it('never relaxes term or campuses and places keywords and subjects last', () => {
    const relaxations = activeRelaxations({
      ...createNeutralFilterState(TERM),
      campuses: ['NB'],
      keywords: ['data'],
      subjects: ['198'],
      permission: 'REQUIRED',
      credits: { minimumHundredths: 300, maximumHundredths: null },
      meetingLocations: { locations: ['CAC'], mode: 'ALL_REQUIRED_MEETINGS' },
      instructors: ['Ada Lovelace'],
      examCodes: ['A'],
      availability: [{ weekday: 'MONDAY', startMinute: 540, endMinute: 600 }],
      sectionIndexes: ['12345'],
    });
    expect(relaxations.map(describeRelaxation)).toEqual([
      'CLEAR_FIELD:availability',
      'CLEAR_FIELD:meetingLocations',
      'CLEAR_FIELD:instructors',
      'CLEAR_FIELD:examCodes',
      'CLEAR_FIELD:permission',
      'CLEAR_FIELD:sectionIndexes',
      'CLEAR_FIELD:credits',
      'CLEAR_FIELD:keywords',
    ]);
    expect(relaxations.map(describeRelaxation)).not.toContain('CLEAR_FIELD:subjects');
  });

  it('neutralises exactly one field and keeps the core and meeting-location modes', () => {
    const filters: FilterStateV1 = {
      ...P1_FILTERS,
      core: { codes: ['QR'], mode: 'ALL' },
      meetingLocations: { locations: ['CAC'], mode: 'ALL_REQUIRED_MEETINGS' },
      credits: { minimumHundredths: 300, maximumHundredths: 400 },
      permission: 'REQUIRED',
    };
    const neutral = createNeutralFilterState(TERM);
    const cases: readonly { readonly relaxation: Relaxation; readonly key: keyof FilterStateV1 }[] = [
      { relaxation: { kind: 'CLEAR_FIELD', field: 'synchronicities', stableId: 'FLT-S04b' }, key: 'synchronicities' },
      { relaxation: { kind: 'CLEAR_FIELD', field: 'courseNumberBands', stableId: 'FLT-C05' }, key: 'courseNumberBands' },
      { relaxation: { kind: 'CLEAR_FIELD', field: 'credits', stableId: 'FLT-C07' }, key: 'credits' },
      { relaxation: { kind: 'CLEAR_FIELD', field: 'prerequisite', stableId: 'FLT-C09' }, key: 'prerequisite' },
      { relaxation: { kind: 'CLEAR_FIELD', field: 'permission', stableId: 'FLT-S10' }, key: 'permission' },
    ];
    for (const { relaxation, key } of cases) {
      const relaxed = relaxFilters(filters, relaxation);
      expect(relaxed[key]).toEqual(neutral[key]);
      const untouched = Object.keys(filters).filter((candidate) => candidate !== key) as (keyof FilterStateV1)[];
      for (const other of untouched) expect(relaxed[other]).toEqual(filters[other]);
    }
    const coreRelaxed = relaxFilters(filters, { kind: 'CLEAR_FIELD', field: 'core', stableId: 'FLT-C08' });
    expect(coreRelaxed.core).toEqual({ codes: [], mode: 'ALL' });
    const locationRelaxed = relaxFilters(filters, { kind: 'CLEAR_FIELD', field: 'meetingLocations', stableId: 'FLT-S07' });
    expect(locationRelaxed.meetingLocations).toEqual({ locations: [], mode: 'ALL_REQUIRED_MEETINGS' });
    expect(locationRelaxed.includeIncomplete).toEqual(filters.includeIncomplete);
  });

  it('turns on exactly one includeIncomplete toggle without touching the filter value', () => {
    const relaxed = relaxValues(P1_FILTERS, { kind: 'INCLUDE_INCOMPLETE', toggle: 'modality', stableId: 'FLT-S04a' });
    expect(relaxed.includeIncomplete).toEqual({ prerequisite: false, modality: true, synchronicity: false });
    expect(relaxed.modalities).toEqual(['ONLINE']);
    expect(relaxed.synchronicities).toEqual(['ASYNC']);
  });

  it('builds a pageSize-1 probe on contract version 3 with the base sort', () => {
    const base: CourseQueryRequestV1 = {
      filters: { contractVersion: 3, values: { ...P1_FILTERS, term: TERM } },
      page: { page: 3, pageSize: 25 },
      sort: { field: 'TITLE', direction: 'ASCENDING' },
    };
    const probe = probeRequest(base, { kind: 'CLEAR_FIELD', field: 'openStatuses', stableId: 'FLT-S03' });
    expect(probe.page).toEqual({ page: 1, pageSize: 1 });
    expect(probe.filters.contractVersion).toBe(3);
    expect(probe.sort).toEqual(base.sort);
    expect(probe.sort).not.toBe(base.sort);
    expect(probe.filters.values.openStatuses).toEqual([]);
    expect(probe.filters.values.modalities).toEqual(['ONLINE']);
    expect(base.filters.values.openStatuses).toEqual(['CLOSED']);
  });
});
