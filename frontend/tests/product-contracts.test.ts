import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import type { CatalogDiscoveryResponseV1 } from '../src/ui/shared/product/contracts/catalog';
import type { OpenObservationV1, OpenRefreshStatusV1 } from '../src/ui/shared/product/contracts/open';
import type {
  CourseQueryRequestV1,
  CourseQueryResponseV1,
  FilterSchemaV1,
} from '../src/ui/shared/product/contracts/query';
import type {
  WatchClientCommandV1,
  WatchServerEventV1,
} from '../src/ui/shared/product/contracts/watch';
import {
  FILTER_FIELD_IDS,
  FILTER_REQUEST_FIELDS,
  createCourseQueryRequestV1,
  createNeutralFilterState,
  serializeFilterRequestV1,
  toFilterValuesV1,
  type FilterStateV1,
} from '../src/ui/shared/product/filters';

function readGolden<T>(name: string): T {
  return JSON.parse(readFileSync(
    new URL(`../../crates/bcsp-contracts/tests/golden/${name}`, import.meta.url),
    'utf8',
  )) as T;
}

describe('Rust query product contracts', () => {
  it('binds the 18-field V3 contract plus additive uncertainty flags to the Rust schema', () => {
    const schema = readGolden<FilterSchemaV1>('filter-schema-v1.json');
    expect(schema.contractVersion).toBe(3);
    expect(schema.fields.map(({ stableId }) => stableId)).toEqual(FILTER_FIELD_IDS);
    expect(schema.fields.map(({ requestField }) => requestField)).toEqual(
      FILTER_REQUEST_FIELDS.filter((field) => field !== 'includeIncomplete'),
    );
    expect(schema.fields.map(({ chipOrder }) => chipOrder)).toEqual(
      Array.from({ length: 18 }, (_, index) => index),
    );
    expect(FILTER_FIELD_IDS).toHaveLength(18);
    expect(FILTER_FIELD_IDS).not.toEqual(expect.arrayContaining([
      'FLT-C10', 'FLT-S02', 'FLT-S08', 'FLT-S11',
    ]));
    expect(FILTER_REQUEST_FIELDS).not.toEqual(expect.arrayContaining([
      'courseLocations', 'sectionNumbers', 'buildingRoom', 'eligibility', 'text',
    ]));

    const neutral = createNeutralFilterState('T2026F');
    const values = toFilterValuesV1(neutral);
    expect(Object.keys(values)).toEqual(FILTER_REQUEST_FIELDS);
    for (const field of schema.fields) {
      if (field.canonicalNeutral.kind === 'JSON') {
        expect(values[field.requestField], field.stableId).toEqual(field.canonicalNeutral.value);
      } else {
        expect(field.requestField).toBe('term');
      }
    }
  });

  it('serializes the representative Rust course-query request byte-for-value', () => {
    const golden = readGolden<CourseQueryRequestV1>('course-query-request-v1.json');
    const state = golden.filters.values as FilterStateV1;
    expect(createCourseQueryRequestV1(state, golden.page, golden.sort)).toEqual(golden);
    expect(JSON.parse(serializeFilterRequestV1(state))).toEqual(golden.filters);
  });

  it('normalizes tokens and enum order exactly once without inventing a term', () => {
    expect(() => toFilterValuesV1(createNeutralFilterState())).toThrow(/term is required/u);
    const state: FilterStateV1 = {
      ...createNeutralFilterState('T2026F'),
      keywords: [' structures ', 'data', 'data'],
      courseNumberBands: [400, 0, 100, 100],
      openStatuses: ['UNKNOWN', 'CLOSED', 'OPEN'],
      modalities: ['HYBRID', 'ONLINE', 'ON_CAMPUS_OR_IN_PERSON'],
      instructors: ['  Ada   Lovelace ', 'Ada Lovelace'],
      availability: [
        { weekday: 'FRIDAY', startMinute: 600, endMinute: 700 },
        { weekday: 'MONDAY', startMinute: 700, endMinute: 800 },
      ],
    };
    const values = toFilterValuesV1(state);
    expect(values.keywords).toEqual(['data', 'structures']);
    expect(values.courseNumberBands).toEqual([0, 100, 400]);
    expect(values.openStatuses).toEqual(['OPEN', 'CLOSED', 'UNKNOWN']);
    expect(values.modalities).toEqual([
      'ON_CAMPUS_OR_IN_PERSON', 'ONLINE', 'HYBRID',
    ]);
    expect(values.instructors).toEqual(['Ada Lovelace']);
    expect(values.availability.map(({ weekday }) => weekday)).toEqual(['MONDAY', 'FRIDAY']);
  });

  it('matches the Rust u32 course-number band boundary exactly', () => {
    const maximumValidBand = 4_294_967_100;
    expect(toFilterValuesV1({
      ...createNeutralFilterState('T2026F'),
      courseNumberBands: [maximumValidBand],
    }).courseNumberBands).toEqual([maximumValidBand]);

    for (const invalidBand of [4_294_967_200, 4_294_967_295, Number.MAX_SAFE_INTEGER]) {
      expect(() => toFilterValuesV1({
        ...createNeutralFilterState('T2026F'),
        courseNumberBands: [invalidBand],
      })).toThrow(/u32-safe nonnegative multiples of 100/u);
    }
  });

  it('reuses the Rust response, discovery, Open, and Watch goldens', () => {
    const response = readGolden<CourseQueryResponseV1>('course-query-response-v1.json');
    const discovery = readGolden<CatalogDiscoveryResponseV1>('catalog-discovery-v1.json');
    const status = readGolden<OpenRefreshStatusV1>('open-refresh-status-v1.json');
    const observation = readGolden<OpenObservationV1>('open-observation-v1.json');
    const start = readGolden<WatchClientCommandV1>('watch-client-start-v1.json');
    const startResult = readGolden<WatchServerEventV1>('watch-server-start-result-v1.json');

    expect(response).toMatchObject({ contractVersion: 3, items: [] });
    expect(discovery.targets[0]?.key).toEqual(status.batch);
    expect(discovery.subjects[0]?.provenance.kind).toBe('DISCOVERY');
    expect(observation.sectionKey).toMatchObject(status.batch);
    expect(start.type).toBe('START_WATCH');
    expect(startResult.type).toBe('START_RESULT');
  });
});
