// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { useState } from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import {
  createNeutralFilterState,
  type CatalogDiscoveryResponseV1,
  type FilterOptionsFieldV2,
  type FilterOptionsResponseV2,
  type FilterSchemaV1,
  type FilterStateV1,
} from '../src/ui/shared/product';
import { FILTER_PANEL_CSS, FilterPanel } from '../src/ui/shared/search/filters';

const SCHEMA = JSON.parse(readFileSync(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
)) as FilterSchemaV1;

const point = {
  contentVersion: 1,
  observationId: '20000000-0000-4000-8000-000000000001',
  observedAt: '2026-07-17T00:00:00Z',
} as const;
const provenance = {
  ...point,
  payloadDigest: 'a'.repeat(64),
  sourceId: 'synthetic-selector',
  sourceKind: 'SELECTOR',
} as const;
const known = (value: string) => ({
  knowledge: 'KNOWN',
  presence: { presence: 'PRESENT', value },
} as const);

const DISCOVERY: CatalogDiscoveryResponseV1 = {
  contractVersion: 1,
  observedAt: point.observedAt,
  sources: [],
  status: {
    availability: 'CURRENT', error: null, isStale: false,
    lastSuccess: point, latestAttempt: point,
  },
  coreCodeDictionaries: [],
  subjects: [{
    code: '198',
    label: known('Computer Science'),
    provenance: { kind: 'DISCOVERY', discovery: provenance },
    target: { campus: 'NB', term: '72026' },
  }],
  targets: [],
};

const OPTIONS: Readonly<Record<FilterOptionsFieldV2, readonly { value: string; label: string }[]>> = {
  KEYWORD: Array.from({ length: 12 }, (_, index) => ({
    value: `keyword-${String(index + 1).padStart(2, '0')}`,
    label: `Keyword ${String(index + 1).padStart(2, '0')}`,
  })),
  COURSE_NUMBER_BAND: Array.from({ length: 12 }, (_, index) => ({
    value: String(index * 100),
    label: `${String(index * 100).padStart(3, '0')}–${String(index * 100 + 99).padStart(3, '0')}`,
  })),
  COURSE_LEVEL: [{ value: 'U', label: 'U' }, { value: 'G', label: 'G' }],
  INSTRUCTOR: [{ value: 'Ada Lovelace', label: 'Ada Lovelace' }],
  MEETING_LOCATION: [{ value: 'CAC', label: 'CAC' }],
  EXAM_CODE: [{ value: 'A', label: 'A' }],
};

function response(field: FilterOptionsFieldV2): FilterOptionsResponseV2 {
  return {
    contractVersion: 3,
    field,
    targetVersions: [{ target: { campus: 'NB', term: '72026' }, contentVersion: 7 }],
    options: OPTIONS[field],
    truncated: false,
  };
}

const loadOptions = async (field: FilterOptionsFieldV2) => response(field);

function Harness({
  initial = { ...createNeutralFilterState('72026'), campuses: ['NB'] },
  locale = 'en-US',
  onSubmit = vi.fn(),
}: {
  readonly initial?: FilterStateV1;
  readonly locale?: 'en-US' | 'zh-CN';
  readonly onSubmit?: () => void;
}) {
  const [value, setValue] = useState(initial);
  return (
    <BcspI18nProvider initialLocale={locale}>
      <FilterPanel
        discovery={DISCOVERY}
        formId="course-filter-form"
        loadOptions={loadOptions}
        onChange={setValue}
        onSubmit={onSubmit}
        schema={SCHEMA}
        searchAvailable
        value={value}
      />
      <output data-testid="state">{JSON.stringify(value)}</output>
    </BcspI18nProvider>
  );
}

afterEach(cleanup);

describe('RC I Round 4 flat 03–18 FilterPanel', () => {
  it('keeps the rail on page scroll while styling only long option lists as accessible scroll regions', () => {
    expect(FILTER_PANEL_CSS).toContain('.filter-panel__subject-list::-webkit-scrollbar');
    expect(FILTER_PANEL_CSS).toContain('.filter-panel__dictionary-options::-webkit-scrollbar');
    expect(FILTER_PANEL_CSS).toContain('overflow-y: auto');
    expect(FILTER_PANEL_CSS).toContain('scrollbar-width: thin');
    expect(FILTER_PANEL_CSS).toContain('scrollbar-color:');
    expect(FILTER_PANEL_CSS).toContain('scrollbar-gutter: stable');
    expect(FILTER_PANEL_CSS).toContain('touch-action: pan-y');
    expect(FILTER_PANEL_CSS).not.toMatch(/\.filter-panel\s*\{[^}]*overflow/);
    expect(FILTER_PANEL_CSS).not.toContain('@keyframes');
    expect(FILTER_PANEL_CSS).not.toContain('transition: all');
  });

  it('renders all 16 rows continuously as 03–18 with no accordion or Search hero', () => {
    const view = render(<Harness />);
    const rows = [...view.container.querySelectorAll('[data-filter-row]')];
    expect(rows).toHaveLength(16);
    expect(rows.map((row) => row.querySelector('.filter-panel__ordinal')?.textContent)).toEqual(
      Array.from({ length: 16 }, (_, index) => String(index + 3).padStart(2, '0')),
    );
    expect(view.container.querySelector('details')).toBeNull();
    expect(view.container.querySelector('[data-filter-fields="03-18"]')).not.toBeNull();
    expect(screen.queryByText('Build a precise search')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Search' })).toBeNull();
  });

  it('uses the HumanTest labels and does not expose technical unknown categories', async () => {
    render(<Harness locale="zh-CN" />);
    expect(screen.getByText('课程号段')).toBeTruthy();
    expect(screen.getByText('课程层次')).toBeTruthy();
    expect(screen.getByRole('radio', { name: '有先修要求' })).toBeTruthy();
    expect(screen.getByRole('radio', { name: '无先修要求' })).toBeTruthy();
    expect(screen.getAllByRole('checkbox', { name: /完整数据显示/u })).toHaveLength(3);
    expect(screen.getByRole('checkbox', { name: '线下' })).toBeTruthy();
    expect(screen.getByRole('checkbox', { name: '在线' })).toBeTruthy();
    expect(screen.getAllByRole('checkbox', { name: '混合' })).toHaveLength(2);
    expect(screen.queryByRole('checkbox', { name: '其他' })).toBeNull();
    expect(screen.queryByRole('checkbox', { name: '未知' })).toBeNull();
    await waitFor(() => expect(screen.getByRole('checkbox', { name: '000级' })).toBeTruthy());
  });

  it('uses plain English labels for course level, prerequisites, format, and timing', async () => {
    render(<Harness />);
    expect(screen.getByText('Course level')).toBeTruthy();
    expect(screen.getByText('Prerequisites')).toBeTruthy();
    expect(screen.getByText('Class format')).toBeTruthy();
    expect(screen.getByText('Meeting timing')).toBeTruthy();
    expect(screen.getByRole('checkbox', { name: 'In person' })).toBeTruthy();
    expect(screen.getByRole('checkbox', { name: 'Synchronous' })).toBeTruthy();
    expect(screen.getByRole('checkbox', { name: 'Asynchronous' })).toBeTruthy();
    await waitFor(() => expect(screen.getByRole('checkbox', { name: '000-level' })).toBeTruthy());
  });

  it('loads every actual course-number band and stores numeric sorted V3 values', async () => {
    render(<Harness />);
    const bandGroup = await screen.findByRole('group', { name: 'Course number band' });
    fireEvent.click(within(bandGroup).getByRole('checkbox', { name: '400-level' }));
    await waitFor(() => {
      const state = JSON.parse(screen.getByTestId('state').textContent ?? '{}') as FilterStateV1;
      expect(state.courseNumberBands).toEqual([400]);
    });
    fireEvent.click(within(bandGroup).getByRole('checkbox', { name: '000-level' }));
    await waitFor(() => {
      const state = JSON.parse(screen.getByTestId('state').textContent ?? '{}') as FilterStateV1;
      expect(state.courseNumberBands).toEqual([0, 400]);
    });
    expect(document.querySelector('[data-filter-chip="FLT-C05"]')?.textContent).toContain('000-level');
  });

  it('keeps every long checkbox option reachable and supports Home/End in searchable dictionaries', async () => {
    render(<Harness />);
    const bandGroup = await screen.findByRole('group', { name: 'Course number band' });
    const bands = within(bandGroup).getAllByRole('checkbox');
    expect(bands).toHaveLength(12);
    expect(bands[0]?.getAttribute('aria-label')).toBe('000-level');
    expect(bands.at(-1)?.getAttribute('aria-label')).toBe('1100-level');
    bands.at(-1)?.focus();
    expect(document.activeElement).toBe(bands.at(-1));

    const keywordInput = screen.getByRole('combobox', { name: 'Keyword match' });
    fireEvent.focus(keywordInput);
    await waitFor(() => expect(keywordInput.getAttribute('aria-expanded')).toBe('true'));
    await waitFor(() => expect(screen.getByRole('option', { name: 'Keyword 12' })).toBeTruthy());
    fireEvent.keyDown(keywordInput, { key: 'End' });
    await waitFor(() => expect(keywordInput.getAttribute('aria-activedescendant')).toMatch(/option-11$/));
    fireEvent.keyDown(keywordInput, { key: 'Home' });
    await waitFor(() => expect(keywordInput.getAttribute('aria-activedescendant')).toMatch(/option-0$/));
  });

  it('keeps every filter neutral by default, including all three incomplete-data switches', () => {
    render(<Harness />);
    const state = JSON.parse(screen.getByTestId('state').textContent ?? '{}') as FilterStateV1;
    expect(state.prerequisite).toBe('ANY');
    expect(state.modalities).toEqual([]);
    expect(state.synchronicities).toEqual([]);
    expect(state.includeIncomplete).toEqual({
      prerequisite: false,
      modality: false,
      synchronicity: false,
    });
  });

  it('implements 09 as a clearable single selection with an independent additive switch', () => {
    render(<Harness />);
    fireEvent.click(screen.getByRole('radio', { name: 'Has prerequisite' }));
    fireEvent.click(screen.getAllByRole('checkbox', { name: /Complete data display/u })[0] as HTMLElement);
    let state = JSON.parse(screen.getByTestId('state').textContent ?? '{}') as FilterStateV1;
    expect(state.prerequisite).toBe('HAS');
    expect(state.includeIncomplete.prerequisite).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: 'Clear selection' }));
    state = JSON.parse(screen.getByTestId('state').textContent ?? '{}') as FilterStateV1;
    expect(state.prerequisite).toBe('ANY');
    expect(state.includeIncomplete.prerequisite).toBe(true);
  });

  it('implements 12 and 13 as OR multi-selects with independent additive switches', () => {
    render(<Harness />);
    fireEvent.click(screen.getByRole('checkbox', { name: 'Online' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Hybrid' }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Synchronous' }));
    const incomplete = screen.getAllByRole('checkbox', { name: /Complete data display/u });
    fireEvent.click(incomplete[1] as HTMLElement);
    fireEvent.click(incomplete[2] as HTMLElement);
    const state = JSON.parse(screen.getByTestId('state').textContent ?? '{}') as FilterStateV1;
    expect(state.modalities).toEqual(['ONLINE', 'HYBRID']);
    expect(state.synchronicities).toEqual(['SYNC']);
    expect(state.includeIncomplete).toEqual({
      prerequisite: false,
      modality: true,
      synchronicity: true,
    });
  });

  it('clears 03–18 to neutral while preserving the applied term and Campus scope', async () => {
    render(<Harness initial={{
      ...createNeutralFilterState('72026'),
      campuses: ['NB', 'CM'],
      levels: ['U'],
      modalities: ['ONLINE'],
      includeIncomplete: { prerequisite: false, modality: true, synchronicity: false },
    }} />);
    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    const state = JSON.parse(screen.getByTestId('state').textContent ?? '{}') as FilterStateV1;
    expect(state.term).toBe('72026');
    expect(state.campuses).toEqual(['NB', 'CM']);
    expect(state.levels).toEqual([]);
    expect(state.modalities).toEqual([]);
    expect(state.includeIncomplete.modality).toBe(false);
  });

  it('submits exactly once through its stable native form', () => {
    const onSubmit = vi.fn();
    render(<Harness onSubmit={onSubmit} />);
    const form = document.getElementById('course-filter-form');
    if (!(form instanceof HTMLFormElement)) throw new Error('Expected native filter form.');
    fireEvent.submit(form);
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });
});
