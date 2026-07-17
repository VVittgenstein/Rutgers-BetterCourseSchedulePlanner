// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { useState } from 'react';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { SupportedLocale } from '../src/ui/shared/i18n/contract';
import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import type {
  CatalogDiscoveryResponseV1,
  FilterOptionsFieldV2,
  FilterOptionsResponseV2,
  FilterSchemaV1,
} from '../src/ui/shared/product';
import {
  FILTER_FIELD_IDS,
  createNeutralFilterState,
  toFilterValuesV1,
  type FilterStateV1,
} from '../src/ui/shared/product';
import { FILTER_PANEL_CSS, FilterPanel } from '../src/ui/shared/search/filters';

const SCHEMA = JSON.parse(readFileSync(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
)) as FilterSchemaV1;

function known(value: string) {
  return { knowledge: 'KNOWN', presence: { presence: 'PRESENT', value } } as const;
}

function discovery(subjectCount = 300): CatalogDiscoveryResponseV1 {
  const provenance = {
    observationId: '20000000-0000-4000-8000-000000000001',
    observedAt: '2026-07-15T00:00:00Z',
    payloadDigest: 'b'.repeat(64),
    sourceId: 'synthetic-selector',
    sourceKind: 'SELECTOR',
  } as const;
  const point = {
    contentVersion: 2,
    observationId: provenance.observationId,
    observedAt: provenance.observedAt,
  } as const;
  return {
    contractVersion: 1,
    observedAt: provenance.observedAt,
    sources: [],
    status: {
      availability: 'CURRENT',
      error: null,
      isStale: false,
      lastSuccess: point,
      latestAttempt: point,
    },
    targets: [
      {
        campusLabel: known('New Brunswick'),
        key: { campus: 'NB', term: 'T2026F' },
        provenance,
        termLabel: known('Fall 2026'),
      },
      {
        campusLabel: known('Newark'),
        key: { campus: 'NK', term: 'T2026F' },
        provenance,
        termLabel: known('Fall 2026'),
      },
    ],
    coreCodeDictionaries: ['NB', 'NK'].map((campus) => {
      const target = { campus, term: 'T2026F' };
      return {
        contentVersion: 2,
        options: [{ code: 'QQ', description: known('Quantitative and Formal Reasoning') }],
        provenance: {
          observationId: provenance.observationId,
          observedAt: provenance.observedAt,
          payloadDigest: provenance.payloadDigest,
          source: 'RUTGERS_CATALOG' as const,
          target,
        },
        target,
      };
    }),
    subjects: Array.from({ length: subjectCount }, (_, index) => {
      const code = `S${String(index).padStart(3, '0')}`;
      return {
        code,
        label: known(`Subject ${index}`),
        provenance: { kind: 'DISCOVERY', discovery: provenance } as const,
        target: { campus: 'NB', term: 'T2026F' },
      };
    }),
  };
}

const OPTION_LABELS: Record<FilterOptionsFieldV2, readonly { value: string; label: string }[]> = {
  KEYWORD: [
    { value: 'data', label: 'data' },
    { value: 'structures', label: 'structures' },
  ],
  COURSE_LEVEL: [
    { value: 'U', label: 'Undergraduate / U' },
    { value: 'G', label: 'Graduate / G' },
  ],
  INSTRUCTOR: [
    { value: 'Smith, Jane', label: 'Smith, Jane' },
    { value: 'Lovelace, Ada', label: 'Lovelace, Ada' },
  ],
  MEETING_LOCATION: [{ value: 'BUSCH', label: 'Busch / BUSCH' }],
  EXAM_CODE: [{ value: 'A', label: 'A / Final exam' }],
};

const loadOptions = vi.fn(async (
  field: FilterOptionsFieldV2,
  query?: string,
): Promise<FilterOptionsResponseV2> => {
  const needle = query?.trim().toLocaleLowerCase() ?? '';
  const options = OPTION_LABELS[field].filter(({ label, value }) =>
    needle.length === 0
    || label.toLocaleLowerCase().includes(needle)
    || value.toLocaleLowerCase().includes(needle));
  return {
    contractVersion: 2,
    field,
    options,
    targetVersions: [{ target: { campus: 'NB', term: 'T2026F' }, contentVersion: 2 }],
    truncated: false,
  };
});

function Harness({
  initial = { ...createNeutralFilterState('T2026F'), campuses: ['NB'] },
  onSubmit = vi.fn(),
  locale = 'en-US',
  catalogDiscovery = discovery(),
  searchAvailable = true,
  disabled = !searchAvailable,
}: {
  readonly initial?: FilterStateV1;
  readonly onSubmit?: () => void;
  readonly locale?: SupportedLocale;
  readonly catalogDiscovery?: CatalogDiscoveryResponseV1;
  readonly searchAvailable?: boolean;
  readonly disabled?: boolean;
}) {
  const [value, setValue] = useState(initial);
  return (
    <BcspI18nProvider initialLocale={locale}>
      <FilterPanel
        disabled={disabled}
        schema={SCHEMA}
        discovery={catalogDiscovery}
        loadOptions={loadOptions}
        value={value}
        onChange={setValue}
        onSubmit={onSubmit}
        searchAvailable={searchAvailable}
      />
      <output data-testid="filter-state">{JSON.stringify(value)}</output>
    </BcspI18nProvider>
  );
}

function state(): FilterStateV1 {
  return JSON.parse(screen.getByTestId('filter-state').textContent ?? '{}') as FilterStateV1;
}

function addToken(label: string, value: string): void {
  fireEvent.change(screen.getByLabelText(label), { target: { value } });
  fireEvent.click(screen.getByRole('button', { name: `Add ${label}` }));
}

afterEach(() => {
  cleanup();
  loadOptions.mockClear();
  document.documentElement.style.removeProperty('--bcsp-navigation-height');
  Object.defineProperty(window, 'scrollY', { configurable: true, value: 0 });
});

describe('controlled 03–18 FilterPanel', () => {
  it('keeps touch, fine-pointer, press, and reduced-motion rules bounded', () => {
    expect(FILTER_PANEL_CSS).toContain('min-width: 2.75rem; min-height: 2.75rem');
    expect(FILTER_PANEL_CSS).toContain('@media (hover: hover) and (pointer: fine)');
    expect(FILTER_PANEL_CSS).toContain('transform: scale(0.97)');
    expect(FILTER_PANEL_CSS).toContain('@media (prefers-reduced-motion: reduce)');
    expect(FILTER_PANEL_CSS).not.toContain('transition: all');
  });

  it('renders the 16 filter rows continuously as 03–18 after scope moved outside the form', () => {
    const view = render(<Harness />);
    const rows = [...view.container.querySelectorAll<HTMLElement>('[data-filter-row]')];

    expect(rows.map((row) => row.dataset.filterRow)).toEqual(
      FILTER_FIELD_IDS.filter((id) => id !== 'FLT-C01' && id !== 'FLT-C02'),
    );
    expect(rows).toHaveLength(16);
    expect([...view.container.querySelectorAll('.filter-panel__ordinal')]
      .map((node) => node.textContent)).toEqual(
      Array.from({ length: 16 }, (_, index) => String(index + 3).padStart(2, '0')),
    );
    for (const retired of ['FLT-C10', 'FLT-S02', 'FLT-S08', 'FLT-S11']) {
      expect(view.container.querySelector(`[data-filter-row="${retired}"]`)).toBeNull();
    }
    expect(screen.queryByLabelText(/Course locations|Section numbers|Building codes|Eligibility/u)).toBeNull();
    expect(screen.getByRole('form', { name: 'Course and Section filters' })).toBeTruthy();
    expect(screen.getByText('Query matrix / 16 channels')).toBeTruthy();
  });

  it('uses the approved Chinese labels without translating Rutgers catalog values', () => {
    render(<Harness locale="zh-CN" />);

    expect(screen.getByRole('form', { name: '课程与课节筛选条件' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '建立精确搜索' })).toBeTruthy();
    expect(screen.getAllByText('关键词匹配').length).toBeGreaterThan(0);
    expect(screen.getByText('Index号索引')).toBeTruthy();
    expect(screen.getByText('子校区')).toBeTruthy();

    fireEvent.change(screen.getByLabelText('搜索学科目录'), {
      target: { value: 'Subject 299' },
    });
    expect(screen.getByRole('checkbox', { name: /S299.*Subject 299/u })).toBeTruthy();
  });

  it('disables dictionaries and submit while the applied target is not ready', () => {
    const view = render(<Harness catalogDiscovery={discovery(0)} searchAvailable={false} />);
    const gate = view.container.querySelector<HTMLFieldSetElement>('.filter-panel__gate');

    expect(gate?.disabled).toBe(true);
    expect(gate?.getAttribute('aria-busy')).toBeNull();
    expect((screen.getByRole('combobox', { name: 'Keyword match' }) as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Search' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getAllByText('Apply a ready query range').length).toBeGreaterThan(0);
    expect(screen.getByText(/Choose at least one ready Campus above/u)).toBeTruthy();
  });

  it('only commits published Keyword and Instructor candidates with case-insensitive lookup', async () => {
    render(<Harness />);

    const keyword = screen.getByRole('combobox', { name: 'Keyword match' });
    fireEvent.change(keyword, { target: { value: 'DAT' } });
    expect(state().keywords).toEqual([]);
    expect(screen.queryByRole('button', { name: /Add Keyword/u })).toBeNull();
    await screen.findByRole('option', { name: 'data' });
    fireEvent.keyDown(keyword, { key: 'Enter' });
    await waitFor(() => expect(state().keywords).toEqual(['data']));

    fireEvent.click(screen.getByText('Same-Section constraints'));
    const instructor = screen.getByRole('combobox', { name: 'Instructor' });
    fireEvent.change(instructor, { target: { value: 'SMI' } });
    expect(state().instructors).toEqual([]);
    await screen.findByRole('option', { name: 'Smith, Jane' });
    expect(loadOptions).toHaveBeenCalledWith('INSTRUCTOR', 'SMI', expect.any(AbortSignal));
    fireEvent.keyDown(instructor, { key: 'Enter' });
    await waitFor(() => expect(state().instructors).toEqual(['Smith, Jane']));

    fireEvent.change(instructor, { target: { value: 'not-published' } });
    await screen.findByText('No published option matches this search.');
    fireEvent.keyDown(instructor, { key: 'Enter' });
    expect(state().instructors).toEqual(['Smith, Jane']);
  });

  it('serializes all retained fields, including dictionary-backed Level, Subcampus, and Exam', async () => {
    render(<Harness />);

    fireEvent.change(screen.getByLabelText('Search subject dictionary'), {
      target: { value: 'Subject 299' },
    });
    fireEvent.click(screen.getByRole('checkbox', { name: /S299.*Subject 299/u }));

    const keyword = screen.getByRole('combobox', { name: 'Keyword match' });
    fireEvent.change(keyword, { target: { value: 'structures' } });
    await screen.findByRole('option', { name: 'structures' });
    fireEvent.keyDown(keyword, { key: 'Enter' });
    addToken('Course numbers', 'cs111');
    fireEvent.click(await screen.findByRole('checkbox', { name: 'U · Undergraduate' }));
    fireEvent.change(screen.getByLabelText('Minimum credits'), { target: { value: '3' } });
    fireEvent.change(screen.getByLabelText('Maximum credits'), { target: { value: '4.5' } });
    fireEvent.change(screen.getByLabelText('Core code match mode'), { target: { value: 'ALL' } });
    fireEvent.click(screen.getByRole('checkbox', { name: /QQ.*Quantitative and Formal Reasoning/u }));
    fireEvent.change(screen.getByLabelText('Prerequisite presence'), { target: { value: 'HAS' } });

    fireEvent.click(screen.getByText('Same-Section constraints'));
    addToken('Section indexes', '12345');
    fireEvent.click(within(screen.getByRole('group', { name: 'Open status' }))
      .getByRole('checkbox', { name: 'Open' }));
    fireEvent.click(within(screen.getByRole('group', { name: 'Modality' }))
      .getByRole('checkbox', { name: 'Online' }));
    fireEvent.click(within(screen.getByRole('group', { name: 'Synchronicity' }))
      .getByRole('checkbox', { name: 'Sync' }));
    const instructor = screen.getByRole('combobox', { name: 'Instructor' });
    fireEvent.change(instructor, { target: { value: 'smi' } });
    await screen.findByRole('option', { name: 'Smith, Jane' });
    fireEvent.keyDown(instructor, { key: 'Enter' });
    fireEvent.change(screen.getByLabelText('Weekday'), { target: { value: 'FRIDAY' } });
    fireEvent.change(screen.getByLabelText('Start'), { target: { value: '10:30' } });
    fireEvent.change(screen.getByLabelText('End'), { target: { value: '12:15' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add window' }));
    fireEvent.change(screen.getByLabelText('Subcampus match mode'), {
      target: { value: 'ALL_REQUIRED_MEETINGS' },
    });
    fireEvent.click(await screen.findByRole('checkbox', { name: 'Busch / BUSCH' }));
    fireEvent.click(await screen.findByRole('checkbox', { name: 'A / Final exam' }));
    fireEvent.change(screen.getByLabelText('Permission requirement'), {
      target: { value: 'REQUIRED' },
    });

    expect(toFilterValuesV1(state())).toEqual({
      term: 'T2026F',
      campuses: ['NB'],
      subjects: ['S299'],
      keywords: ['structures'],
      courseNumbers: ['CS111'],
      levels: ['U'],
      credits: { minimumHundredths: 300, maximumHundredths: 450 },
      core: { codes: ['QQ'], mode: 'ALL' },
      prerequisite: 'HAS',
      sectionIndexes: ['12345'],
      openStatuses: ['OPEN'],
      modalities: ['ONLINE'],
      synchronicities: ['SYNC'],
      instructors: ['Smith, Jane'],
      availability: [{ weekday: 'FRIDAY', startMinute: 630, endMinute: 735 }],
      meetingLocations: { locations: ['BUSCH'], mode: 'ALL_REQUIRED_MEETINGS' },
      examCodes: ['A'],
      permission: 'REQUIRED',
    });
  }, 10_000);

  it('keeps Core choices dictionary-only, removes the old search box, and preserves ANY/ALL', () => {
    render(<Harness />);

    expect(screen.queryByLabelText('Search published Core codes')).toBeNull();
    expect(screen.getByRole('group', { name: 'Published Core codes' })).toBeTruthy();
    fireEvent.click(screen.getByRole('checkbox', { name: /QQ.*Quantitative and Formal Reasoning/u }));
    fireEvent.change(screen.getByLabelText('Core code match mode'), { target: { value: 'ALL' } });
    expect(state().core).toEqual({ codes: ['QQ'], mode: 'ALL' });
    fireEvent.change(screen.getByLabelText('Core code match mode'), { target: { value: 'ANY' } });
    expect(state().core).toEqual({ codes: ['QQ'], mode: 'ANY' });
  });

  it('shows and explicitly removes incompatible Saved View Core values', () => {
    const view = render(<Harness initial={{
      ...createNeutralFilterState('T2026F'),
      campuses: ['NB'],
      core: { codes: ['LEGACY'], mode: 'ALL' },
      keywords: ['data'],
      levels: ['U'],
      instructors: ['Smith, Jane'],
      meetingLocations: { locations: ['BUSCH'], mode: 'ALL_REQUIRED_MEETINGS' },
      examCodes: ['A'],
    }} />);

    expect(screen.getByText('Saved incompatible Core codes')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Remove incompatible Core code LEGACY' }));
    expect(state().core).toEqual({ codes: [], mode: 'ALL' });

    expect(state()).toMatchObject({
      campuses: ['NB'],
      keywords: ['data'],
      levels: ['U'],
      instructors: ['Smith, Jane'],
      meetingLocations: { locations: ['BUSCH'], mode: 'ALL_REQUIRED_MEETINGS' },
      examCodes: ['A'],
    });
    expect(view.container.querySelector('[data-filter-row="FLT-S07"]')).not.toBeNull();
  });

  it('moves from 03–09 to 10–18 inside the desktop rail, focuses the new summary, and preserves values', async () => {
    const view = render(
      <div className="bcsp-search-workspace__filters" style={{ overflowY: 'auto' }}>
        <Harness />
      </div>,
    );
    const rail = view.container.querySelector<HTMLElement>('.bcsp-search-workspace__filters');
    if (rail === null) throw new Error('filter rail is required');
    Object.defineProperties(rail, {
      clientHeight: { configurable: true, value: 400 },
      scrollHeight: { configurable: true, value: 1200 },
    });
    Object.defineProperty(rail, 'getBoundingClientRect', {
      configurable: true,
      value: () => ({
        top: 100, bottom: 500, left: 0, right: 500, width: 500, height: 400,
        x: 0, y: 100, toJSON() {},
      }),
    });
    const stickyHeader = view.container.querySelector<HTMLElement>('.filter-panel__head');
    if (stickyHeader === null) throw new Error('sticky filter header is required');
    Object.defineProperty(stickyHeader, 'getBoundingClientRect', {
      configurable: true,
      value: () => ({
        top: 100, bottom: 180, left: 0, right: 500, width: 500, height: 80,
        x: 0, y: 100, toJSON() {},
      }),
    });

    addToken('Course numbers', '198111');
    const sectionSummary = screen.getByText('Same-Section constraints').closest('summary');
    if (sectionSummary === null) throw new Error('Section summary is required');
    Object.defineProperty(sectionSummary, 'getBoundingClientRect', {
      configurable: true,
      value: () => ({
        top: 360, bottom: 410, left: 0, right: 500, width: 500, height: 50,
        x: 0, y: 360, toJSON() {},
      }),
    });

    fireEvent.click(sectionSummary);
    await waitFor(() => {
      expect(document.activeElement).toBe(sectionSummary);
      expect(rail.scrollTop).toBe(180);
    });
    expect(state().courseNumbers).toEqual(['198111']);
    expect((sectionSummary.parentElement as HTMLDetailsElement).open).toBe(true);
    expect(screen.getByText('Course constraints').closest('details')?.hasAttribute('open')).toBe(false);
  });

  it.each(['Enter', ' '])(
    'returns from 10–18 to 03–09 with %s keyboard activation and a sticky-safe window position',
    async (key) => {
      const scrollTo = vi.spyOn(window, 'scrollTo').mockImplementation(() => undefined);
      document.documentElement.style.setProperty('--bcsp-navigation-height', '64px');
      Object.defineProperty(window, 'scrollY', { configurable: true, value: 100 });
      render(<Harness />);

      const sectionSummary = screen.getByText('Same-Section constraints').closest('summary');
      const courseSummary = screen.getByText('Course constraints').closest('summary');
      if (sectionSummary === null || courseSummary === null) {
        throw new Error('both summaries are required');
      }
      fireEvent.click(sectionSummary);
      await waitFor(() => expect(document.activeElement).toBe(sectionSummary));
      Object.defineProperty(courseSummary, 'getBoundingClientRect', {
        configurable: true,
        value: () => ({
          top: 400, bottom: 450, left: 0, right: 500, width: 500, height: 50,
          x: 0, y: 400, toJSON() {},
        }),
      });

      fireEvent.keyDown(courseSummary, { key });
      (courseSummary.parentElement as HTMLDetailsElement).open = true;
      fireEvent(courseSummary.parentElement as HTMLDetailsElement, new Event('toggle'));
      await waitFor(() => {
        expect(document.activeElement).toBe(courseSummary);
        expect(scrollTo).toHaveBeenLastCalledWith({ behavior: 'auto', top: 436 });
      });
      expect(screen.getByText('Same-Section constraints').closest('details')?.hasAttribute('open')).toBe(false);
      document.documentElement.style.removeProperty('--bcsp-navigation-height');
    },
  );

  it.each(['mouse', 'Enter', ' '] as const)(
    'supports %s activation in both 03–09 → 10–18 and 10–18 → 03–09 directions',
    async (method) => {
      vi.spyOn(window, 'scrollTo').mockImplementation(() => undefined);
      render(<Harness />);
      const sectionSummary = screen.getByText('Same-Section constraints').closest('summary');
      const courseSummary = screen.getByText('Course constraints').closest('summary');
      if (sectionSummary === null || courseSummary === null) {
        throw new Error('both summaries are required');
      }
      const activate = (summary: HTMLElement) => {
        if (method === 'mouse') {
          fireEvent.click(summary);
          return;
        }
        fireEvent.keyDown(summary, { key: method });
        (summary.parentElement as HTMLDetailsElement).open = true;
        fireEvent(summary.parentElement as HTMLDetailsElement, new Event('toggle'));
      };

      activate(sectionSummary);
      await waitFor(() => expect(document.activeElement).toBe(sectionSummary));
      expect((sectionSummary.parentElement as HTMLDetailsElement).open).toBe(true);
      expect((courseSummary.parentElement as HTMLDetailsElement).open).toBe(false);

      activate(courseSummary);
      await waitFor(() => expect(document.activeElement).toBe(courseSummary));
      expect((courseSummary.parentElement as HTMLDetailsElement).open).toBe(true);
      expect((sectionSummary.parentElement as HTMLDetailsElement).open).toBe(false);
    },
  );

  it('adds/removes an availability window and submits once while preserving the target', () => {
    const onSubmit = vi.fn();
    render(<Harness onSubmit={onSubmit} />);

    fireEvent.click(screen.getByText('Same-Section constraints'));
    fireEvent.change(screen.getByLabelText('Weekday'), { target: { value: 'FRIDAY' } });
    fireEvent.change(screen.getByLabelText('Start'), { target: { value: '10:30' } });
    fireEvent.change(screen.getByLabelText('End'), { target: { value: '12:15' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add window' }));
    expect(state().availability).toEqual([
      { weekday: 'FRIDAY', startMinute: 630, endMinute: 735 },
    ]);
    fireEvent.click(screen.getByRole('button', { name: 'Remove availability window 1' }));
    expect(state().availability).toEqual([]);

    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(state()).toMatchObject({ term: 'T2026F', campuses: ['NB'] });
  });
});
