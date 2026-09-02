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

/** The server dictionary publishes mixed-case Core codes (AHo, WCd, …) while the
 * canonical request form stores them uppercased (AHO, WCD). */
const CORE_DISCOVERY: CatalogDiscoveryResponseV1 = {
  ...DISCOVERY,
  coreCodeDictionaries: [{
    target: { campus: 'NB', term: '72026' },
    contentVersion: 7,
    provenance: {
      observationId: point.observationId,
      source: 'RUTGERS_CATALOG',
      target: { campus: 'NB', term: '72026' },
      observedAt: point.observedAt,
      payloadDigest: 'b'.repeat(64),
    },
    options: [
      { code: 'AHo', description: known('Arts and Humanities (o)') },
      { code: 'WCd', description: known('Writing and Communication (d)') },
      { code: 'QQ', description: known('Quantitative Reasoning') },
    ],
  }],
};

function Harness({
  discovery = DISCOVERY,
  initial = { ...createNeutralFilterState('72026'), campuses: ['NB'] },
  locale = 'en-US',
  onSubmit = vi.fn(),
}: {
  readonly discovery?: CatalogDiscoveryResponseV1;
  readonly initial?: FilterStateV1;
  readonly locale?: 'en-US' | 'zh-CN';
  readonly onSubmit?: () => void;
}) {
  const [value, setValue] = useState(initial);
  return (
    <BcspI18nProvider initialLocale={locale}>
      <FilterPanel
        discovery={discovery}
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
    // Spec v2 section 11.2: scrollbars stay hidden until hover / focus-within and never reserve a gutter.
    expect(FILTER_PANEL_CSS).toContain('scrollbar-width: none');
    expect(FILTER_PANEL_CSS).toMatch(/\.filter-panel__subject-list:(?:hover|focus-within)[^{]*\{[^}]*scrollbar-width:\s*thin/u);
    expect(FILTER_PANEL_CSS).toMatch(/\.filter-panel__dictionary-options:(?:hover|focus-within)[^{]*\{[^}]*scrollbar-width:\s*thin/u);
    expect(FILTER_PANEL_CSS).toContain('scrollbar-color:');
    expect(FILTER_PANEL_CSS).not.toContain('scrollbar-gutter: stable');
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

  it('shows the 16 rows as four titled groups that each hold at least one row', () => {
    const view = render(<Harness />);
    const groups = [...view.container.querySelectorAll('[data-filter-fields="03-18"] > [data-filter-group]')];
    expect(groups.map((group) => group.getAttribute('data-filter-group'))).toEqual([
      'course', 'requirements', 'sections', 'time-place',
    ]);
    expect(groups.map((group) => group.querySelector('.filter-panel__group-title')?.textContent)).toEqual([
      'Course', 'Requirements', 'Sections', 'Time, place and other',
    ]);
    for (const group of groups) {
      expect(group.getAttribute('role')).toBe('group');
      expect(group.querySelectorAll('[data-filter-row]').length).toBeGreaterThan(0);
    }
    expect(groups.flatMap((group) => [...group.querySelectorAll('.filter-panel__ordinal')].map((ordinal) => ordinal.textContent)))
      .toEqual(Array.from({ length: 16 }, (_, index) => String(index + 3).padStart(2, '0')));
    expect(view.container.querySelector('.filter-panel__active')?.getAttribute('data-count')).toBe('0');
    expect(view.container.querySelector('[data-filter-group="course"]')?.getAttribute('data-count')).toBe('0');
    cleanup();

    render(<Harness locale="zh-CN" />);
    expect([...document.querySelectorAll('.filter-panel__group-title')].map((title) => title.textContent)).toEqual([
      '课程', '要求', '课节', '时间、地点与其他',
    ]);
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

  it('explains the three meeting-timing options and their incomplete-data switch in English', () => {
    const view = render(<Harness />);
    const row = view.container.querySelector<HTMLElement>('[data-filter-row="FLT-S04b"]');
    if (row === null) throw new Error('Expected the Meeting timing row.');
    expect(within(row).getByText('Derived from the Rutgers meeting mode and the listed meeting times.')).toBeTruthy();
    expect(within(row).getByText('Meets at fixed times (online or in person).')).toBeTruthy();
    expect(within(row).getByText('Online with no fixed meeting time (Rutgers “hours by arrangement”).')).toBeTruthy();
    expect(within(row).getByText('Some meetings at fixed times, some by arrangement.')).toBeTruthy();
    expect(within(row).getByText('Also include sections whose meeting timing Rutgers has not published.')).toBeTruthy();
    expect(within(row).queryByText('When this filter is active, also include records whose value cannot be determined.')).toBeNull();
    expect(within(row).getByRole('checkbox', { name: 'Synchronous' })).toBeTruthy();
    expect(within(row).getByRole('checkbox', { name: 'Asynchronous' })).toBeTruthy();
    expect(within(row).getByRole('checkbox', { name: 'Mixed' })).toBeTruthy();
    expect(within(row).getByRole('checkbox', { name: /Complete data display/u })).toBeTruthy();

    const formatRow = view.container.querySelector<HTMLElement>('[data-filter-row="FLT-S04a"]');
    if (formatRow === null) throw new Error('Expected the Class format row.');
    expect(within(formatRow).getByText('When this filter is active, also include records whose value cannot be determined.')).toBeTruthy();
    expect(within(formatRow).queryByText(/hours by arrangement/u)).toBeNull();
  });

  it('explains the three meeting-timing options in Chinese', () => {
    const view = render(<Harness locale="zh-CN" />);
    const row = view.container.querySelector<HTMLElement>('[data-filter-row="FLT-S04b"]');
    if (row === null) throw new Error('Expected the Meeting timing row.');
    expect(within(row).getByText('根据 Rutgers 的授课模式与列出的上课时间推断。')).toBeTruthy();
    expect(within(row).getByText('有固定上课时间（线上或线下）。')).toBeTruthy();
    expect(within(row).getByText('在线且没有固定上课时间（Rutgers 标注 hours by arrangement）。')).toBeTruthy();
    expect(within(row).getByText('部分时段固定，部分自行安排。')).toBeTruthy();
    expect(within(row).getByText('同时包含 Rutgers 未公布上课时间安排的课节。')).toBeTruthy();
    expect(within(row).getByRole('checkbox', { name: '混合' })).toBeTruthy();
    expect(within(row).getByRole('checkbox', { name: /完整数据显示/u })).toBeTruthy();
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

describe('FilterPanel Core codes are case-insensitive', () => {
  const coreState = (codes: readonly string[]): FilterStateV1 => ({
    ...createNeutralFilterState('72026'),
    campuses: ['NB'],
    core: { codes: [...codes], mode: 'ANY' },
  });
  const readCodes = () =>
    (JSON.parse(screen.getByTestId('state').textContent ?? '{}') as FilterStateV1).core.codes;
  const AHO_LABEL = 'AHo · Arts and Humanities (o)';

  it('renders a persisted uppercase code as checked, compatible, and labelled from the dictionary', () => {
    render(<Harness discovery={CORE_DISCOVERY} initial={coreState(['AHO'])} />);
    const group = screen.getByRole('group', { name: 'Published Core codes' });
    const aho = within(group).getByRole('checkbox', { name: AHO_LABEL });
    expect((aho as HTMLInputElement).checked).toBe(true);
    expect((within(group).getByRole('checkbox', { name: /^WCd/u }) as HTMLInputElement).checked).toBe(false);
    expect(screen.queryByText('Saved incompatible Core codes')).toBeNull();
    expect(screen.queryByRole('button', { name: /Remove incompatible Core code/u })).toBeNull();
    const chip = document.querySelector('[data-filter-chip="FLT-C08"]')?.textContent ?? '';
    expect(chip).toContain(AHO_LABEL);
    expect(chip).not.toMatch(/AHO/u);
    expect(readCodes()).toEqual(['AHO']);
  });

  it('toggling a checked dictionary code off removes the persisted uppercase value', () => {
    render(<Harness discovery={CORE_DISCOVERY} initial={coreState(['AHO', 'WCD'])} />);
    const group = screen.getByRole('group', { name: 'Published Core codes' });
    fireEvent.click(within(group).getByRole('checkbox', { name: AHO_LABEL }));
    expect(readCodes()).toEqual(['WCD']);
    expect((within(group).getByRole('checkbox', { name: AHO_LABEL }) as HTMLInputElement).checked).toBe(false);
    expect(document.querySelector('[data-filter-chip="FLT-C08"]')?.textContent).not.toContain('AHo');
  });

  it('toggling a dictionary code on stores exactly one canonical uppercase value', () => {
    render(<Harness discovery={CORE_DISCOVERY} initial={coreState([])} />);
    const group = screen.getByRole('group', { name: 'Published Core codes' });
    fireEvent.click(within(group).getByRole('checkbox', { name: AHO_LABEL }));
    expect(readCodes()).toEqual(['AHO']);
    fireEvent.click(within(group).getByRole('checkbox', { name: /^WCd/u }));
    expect(readCodes()).toEqual(['AHO', 'WCD']);
    expect((within(group).getByRole('checkbox', { name: AHO_LABEL }) as HTMLInputElement).checked).toBe(true);
    expect(screen.queryByText('Saved incompatible Core codes')).toBeNull();
  });

  it('collapses legacy mixed-case duplicates instead of storing AHo and AHO side by side', () => {
    render(<Harness discovery={CORE_DISCOVERY} initial={coreState(['AHo', 'AHO', 'QQ'])} />);
    const group = screen.getByRole('group', { name: 'Published Core codes' });
    const aho = within(group).getByRole('checkbox', { name: AHO_LABEL });
    expect((aho as HTMLInputElement).checked).toBe(true);
    expect(screen.queryByText('Saved incompatible Core codes')).toBeNull();
    const chip = document.querySelector('[data-filter-chip="FLT-C08"]')?.textContent ?? '';
    expect(chip.match(/Arts and Humanities/gu)).toHaveLength(1);
    fireEvent.click(aho);
    expect(readCodes()).toEqual(['QQ']);
    fireEvent.click(within(group).getByRole('checkbox', { name: AHO_LABEL }));
    expect(readCodes()).toEqual(['QQ', 'AHO']);
  });

  it('detects incompatible codes by canonical form and removes every case variant at once', () => {
    render(<Harness discovery={CORE_DISCOVERY} initial={coreState(['AHO', 'zzq', 'ZZQ'])} />);
    expect(screen.getByText('Saved incompatible Core codes')).toBeTruthy();
    const removeButtons = screen.getAllByRole('button', { name: /Remove incompatible Core code/u });
    expect(removeButtons).toHaveLength(1);
    expect(removeButtons[0]?.getAttribute('aria-label')).toBe('Remove incompatible Core code zzq');
    fireEvent.click(removeButtons[0] as HTMLElement);
    expect(readCodes()).toEqual(['AHO']);
    expect(screen.queryByText('Saved incompatible Core codes')).toBeNull();
  });
});
