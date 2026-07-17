// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useState } from 'react';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import {
  FILTER_FIELD_IDS,
  createNeutralFilterState,
  isSearchDataReady,
  type CatalogDiscoveryResponseV1,
  type FilterOptionsFieldV2,
  type FilterSchemaV1,
  type ServiceStatusV2,
} from '../src/ui/shared/product';
import { FilterPanel } from '../src/ui/shared/search/filters';
import { SEARCH_WORKSPACE_CSS } from '../src/ui/shared/search/searchStyles';

const rawSchema = JSON.parse(readFileSync(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
)) as FilterSchemaV1;
const active = new Set(FILTER_FIELD_IDS);
const schema = {
  contractVersion: 2,
  fields: rawSchema.fields
    .filter(({ stableId }) => active.has(stableId as (typeof FILTER_FIELD_IDS)[number]))
    .map((field, chipOrder) => ({
      ...field,
      chipOrder,
      requestField: (field.requestField as string) === 'text' ? 'keywords' : field.requestField,
    })),
} as FilterSchemaV1;

const known = (value: string) => ({ knowledge: 'KNOWN', presence: { presence: 'PRESENT', value } } as const);
const discovery: CatalogDiscoveryResponseV1 = {
  contractVersion: 1,
  coreCodeDictionaries: [],
  observedAt: '2026-07-16T00:00:00Z',
  sources: [],
  status: {
    availability: 'CURRENT', error: null, isStale: false, lastSuccess: null, latestAttempt: null,
  },
  subjects: [],
  targets: [{
    campusLabel: known('New Brunswick'),
    key: { campus: 'NB', term: '92026' },
    provenance: {
      observationId: '10000000-0000-4000-8000-000000000001',
      observedAt: '2026-07-16T00:00:00Z',
      payloadDigest: 'a'.repeat(64),
      sourceId: 'selector',
      sourceKind: 'SELECTOR',
    },
    termLabel: known('Fall 2026'),
  }],
};

afterEach(cleanup);

describe('RC3 course workspace contract', () => {
  it('renders exactly 16 search filters after the two scope controls and disables only 03–18', () => {
    const view = render(
      <BcspI18nProvider initialLocale="en-US">
        <FilterPanel
          disabled
          discovery={discovery}
          onChange={vi.fn()}
          onSubmit={vi.fn()}
          schema={schema}
          searchAvailable={false}
          value={{ ...createNeutralFilterState('92026'), campuses: ['NB'] }}
        />
      </BcspI18nProvider>,
    );
    expect(view.container.querySelectorAll('[data-filter-row]')).toHaveLength(16);
    expect(screen.queryByRole('combobox', { name: 'Term' })).toBeNull();
    expect(screen.queryByRole('group', { name: 'Campus' })).toBeNull();
    expect(screen.getByRole('combobox', { name: 'Keyword match' }).matches(':disabled')).toBe(true);
    expect(screen.getAllByText('Apply a ready query range').length).toBeGreaterThan(0);
    expect(view.container.querySelector('[data-filter-row="FLT-C10"]')).toBeNull();
    expect(view.container.querySelector('[data-filter-row="FLT-S02"]')).toBeNull();
    expect(view.container.querySelector('[data-filter-row="FLT-S08"]')).toBeNull();
    expect(view.container.querySelector('[data-filter-row="FLT-S11"]')).toBeNull();
  });

  it('uses a case-insensitive searchable dictionary and keyboard selection for instructors', async () => {
    const loadOptions = vi.fn(async (field: FilterOptionsFieldV2, query?: string) => ({
      contractVersion: 2 as const,
      field,
      options: field === 'INSTRUCTOR' && query?.toLocaleLowerCase() === 'smi'
        ? [{ value: 'Smith, Jane', label: 'Smith, Jane' }]
        : [],
      targetVersions: [],
      truncated: false,
    }));
    function Controlled() {
      const [value, setValue] = useState({ ...createNeutralFilterState('92026'), campuses: ['NB'] });
      return <FilterPanel discovery={discovery} loadOptions={loadOptions} onChange={setValue}
        onSubmit={vi.fn()} schema={schema} value={value} />;
    }
    render(<BcspI18nProvider initialLocale="en-US"><Controlled /></BcspI18nProvider>);
    const instructor = screen.getByRole('combobox', { name: 'Instructor' });
    fireEvent.change(instructor, { target: { value: 'SMI' } });
    await screen.findByRole('option', { name: /Smith, Jane/u });
    expect(loadOptions).toHaveBeenCalledWith('INSTRUCTOR', 'SMI', expect.any(AbortSignal));
    fireEvent.keyDown(instructor, { key: 'Enter' });
    await waitFor(() => expect(screen.getAllByText('Smith, Jane').length).toBeGreaterThan(0));
  });

  it('localizes published U/G course-level values in the active interface language', async () => {
    const loadOptions = vi.fn(async (field: FilterOptionsFieldV2) => ({
      contractVersion: 2 as const,
      field,
      options: field === 'COURSE_LEVEL'
        ? [
            { value: 'U', label: 'U · Undergraduate' },
            { value: 'G', label: 'G · Graduate' },
          ]
        : [],
      targetVersions: [],
      truncated: false,
    }));
    render(
      <BcspI18nProvider initialLocale="zh-CN">
        <FilterPanel
          discovery={discovery}
          loadOptions={loadOptions}
          onChange={vi.fn()}
          onSubmit={vi.fn()}
          schema={schema}
          value={{ ...createNeutralFilterState('92026'), campuses: ['NB'] }}
        />
      </BcspI18nProvider>,
    );
    expect(await screen.findByText('U · 本科')).toBeTruthy();
    expect(await screen.findByText('G · 研究生')).toBeTruthy();
  });

  it('gates only the exact applied V2 targets and keeps READY plus retry usable', () => {
    const status = {
      contractVersion: 2,
      automaticTermSummaries: [{ term: '92026', readyTargetCount: 1, totalTargetCount: 2 }],
      targets: [
        { target: { term: '92026', campus: 'NB' }, usable: true, workState: 'RETRY_WAIT' },
        { target: { term: '92026', campus: 'NK' }, usable: false, workState: 'RETRY_WAIT' },
      ],
      issues: [],
    } as unknown as ServiceStatusV2;
    expect(isSearchDataReady(undefined)).toBe(false);
    expect(isSearchDataReady(status)).toBe(false);
    expect(isSearchDataReady(status, { term: '92026', campuses: ['NB'] })).toBe(true);
    expect(isSearchDataReady(status, { term: '92026', campuses: ['NB', 'NK'] })).toBe(false);
    expect(SEARCH_WORKSPACE_CSS).toContain('clamp(26rem, 28vw, 34rem)');
  });
});
