// @vitest-environment jsdom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { useState } from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import type { SupportedLocale } from '../src/ui/shared/i18n/contract';
import type {
  CatalogDiscoveryResponseV1,
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

function Harness({
  initial = { ...createNeutralFilterState('T2026F'), campuses: ['NB'] },
  onSubmit = vi.fn(),
  locale = 'en-US',
}: {
  readonly initial?: FilterStateV1;
  readonly onSubmit?: () => void;
  readonly locale?: SupportedLocale;
}) {
  const [value, setValue] = useState(initial);
  return (
    <BcspI18nProvider initialLocale={locale}>
      <FilterPanel
        schema={SCHEMA}
        discovery={discovery()}
        value={value}
        onChange={setValue}
        onSubmit={onSubmit}
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

afterEach(() => cleanup());

describe('controlled 22-field FilterPanel', () => {
  it('keeps removable chip targets touch-sized with fine-pointer hover and reduced-motion press handling', () => {
    expect(FILTER_PANEL_CSS).toContain('min-width: 2.75rem; min-height: 2.75rem');
    expect(FILTER_PANEL_CSS).toContain('@media (hover: hover) and (pointer: fine)');
    expect(FILTER_PANEL_CSS).toContain('transform: scale(0.97)');
    expect(FILTER_PANEL_CSS).toContain('@media (prefers-reduced-motion: reduce)');
  });

  it('renders every stable schema row in chip order with a real form control', () => {
    const view = render(<Harness />);
    const rows = [...view.container.querySelectorAll<HTMLElement>('[data-filter-row]')];

    expect(rows.map((row) => row.dataset.filterRow)).toEqual(FILTER_FIELD_IDS);
    expect(rows).toHaveLength(22);
    for (const row of rows) {
      expect(row.querySelector('input, select, button'), row.dataset.filterRow).not.toBeNull();
    }
    expect(screen.getByRole('form', { name: 'Course and Section filters' })).toBeTruthy();
    expect(screen.getByText('Query matrix / 22 channels')).toBeTruthy();
  });

  it('translates the Chinese filter chrome without translating Rutgers catalog values', () => {
    render(<Harness locale="zh-CN" />);

    expect(screen.getByRole('form', { name: '课程与课节筛选条件' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: '建立精确搜索' })).toBeTruthy();
    expect(screen.getByLabelText('学期')).toBeTruthy();
    expect(screen.getByRole('option', { name: 'Fall 2026 / T2026F' })).toBeTruthy();

    fireEvent.change(screen.getByLabelText('搜索学科目录'), {
      target: { value: 'Subject 299' },
    });
    expect(screen.getByRole('checkbox', { name: 'S299 · Subject 299' })).toBeTruthy();
  });

  it('interacts with and serializes every one of the 22 schema fields', () => {
    render(<Harness />);

    // C01-C03: exercise the published target and dictionary controls themselves,
    // rather than relying on the Harness defaults as evidence of coverage.
    fireEvent.change(screen.getByLabelText('Term'), { target: { value: '' } });
    fireEvent.change(screen.getByLabelText('Term'), { target: { value: 'T2026F' } });
    fireEvent.click(within(screen.getByRole('group', { name: 'Campus' }))
      .getByRole('checkbox', { name: /Newark/ }));
    fireEvent.change(screen.getByLabelText('Search subject dictionary'), {
      target: { value: 'Subject 299' },
    });
    fireEvent.click(screen.getByRole('checkbox', { name: /S299.*Subject 299/ }));

    // C04-C10: all Course constraints are visible in the default COURSES mode.
    fireEvent.change(screen.getByLabelText('Search text'), {
      target: { value: '  data   structures  ' },
    });
    addToken('Course numbers', '  cs111  ');
    addToken('Course levels', '  ug  ');
    fireEvent.change(screen.getByLabelText('Minimum credits'), { target: { value: '3' } });
    fireEvent.change(screen.getByLabelText('Maximum credits'), { target: { value: '4.5' } });
    fireEvent.change(screen.getByLabelText('Core code match mode'), { target: { value: 'ALL' } });
    addToken('Core codes', '  qq  ');
    fireEvent.change(screen.getByLabelText('Prerequisite presence'), { target: { value: 'HAS' } });
    addToken('Course locations', '  liv  ');

    // The accordion is single-open. Move to the Section group before exercising S01-S11.
    fireEvent.click(screen.getByText('Same-Section constraints'));
    addToken('Section indexes', '12345');
    addToken('Section numbers', '  a1  ');
    fireEvent.click(within(screen.getByRole('group', { name: 'Open status' }))
      .getByRole('checkbox', { name: 'Open' }));
    fireEvent.click(within(screen.getByRole('group', { name: 'Modality' }))
      .getByRole('checkbox', { name: 'Online' }));
    fireEvent.click(within(screen.getByRole('group', { name: 'Synchronicity' }))
      .getByRole('checkbox', { name: 'Sync' }));
    addToken('Instructor names', '  Jane   Doe  ');
    fireEvent.change(screen.getByLabelText('Weekday'), { target: { value: 'FRIDAY' } });
    fireEvent.change(screen.getByLabelText('Start'), { target: { value: '10:30' } });
    fireEvent.change(screen.getByLabelText('End'), { target: { value: '12:15' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add window' }));
    addToken('Meeting locations', '  busch  ');
    addToken('Building codes', '  hill  ');
    addToken('Room numbers', '  114  ');
    addToken('Exam codes', '  a  ');
    fireEvent.change(screen.getByLabelText('Permission requirement'), {
      target: { value: 'REQUIRED' },
    });
    addToken('Major codes', '  cs  ');
    addToken('Minor codes', '  math  ');
    addToken('Honors program codes', '  sas  ');
    addToken('Unit codes', '  01  ');
    fireEvent.change(screen.getByLabelText('Unit code for pair'), { target: { value: '  01  ' } });
    fireEvent.change(screen.getByLabelText('Major code for pair'), { target: { value: '  cs  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add pair' }));

    expect(toFilterValuesV1(state())).toEqual({
      term: 'T2026F',
      campuses: ['NB', 'NK'],
      subjects: ['S299'],
      text: 'data structures',
      courseNumbers: ['CS111'],
      levels: ['UG'],
      credits: { minimumHundredths: 300, maximumHundredths: 450 },
      core: { codes: ['QQ'], mode: 'ALL' },
      prerequisite: 'HAS',
      courseLocations: ['LIV'],
      sectionIndexes: ['12345'],
      sectionNumbers: ['A1'],
      openStatuses: ['OPEN'],
      modalities: ['ONLINE'],
      synchronicities: ['SYNC'],
      instructors: ['Jane Doe'],
      availability: [{ weekday: 'FRIDAY', startMinute: 630, endMinute: 735 }],
      meetingLocations: ['BUSCH'],
      buildingRoom: { buildingCodes: ['HILL'], roomNumbers: ['114'] },
      examCodes: ['A'],
      permission: 'REQUIRED',
      eligibility: {
        majorCodes: ['CS'],
        minorCodes: ['MATH'],
        honorProgramCodes: ['SAS'],
        unitCodes: ['01'],
        unitMajors: [{ unitCode: '01', majorCode: 'CS' }],
      },
    });
  });

  it('searches a large published subject dictionary and combines filters into clearable chips', () => {
    render(<Harness />);

    expect(screen.getByText('300 subjects shown')).toBeTruthy();
    fireEvent.change(screen.getByLabelText('Search subject dictionary'), {
      target: { value: 'Subject 299' },
    });
    expect(screen.getByText('1 subjects shown')).toBeTruthy();
    fireEvent.click(screen.getByRole('checkbox', { name: 'S299 · Subject 299' }));

    fireEvent.change(screen.getByLabelText('Search text'), { target: { value: 'data structures' } });
    fireEvent.click(screen.getByText('Same-Section constraints'));
    fireEvent.click(within(screen.getByRole('group', { name: 'Open status' }))
      .getByRole('checkbox', { name: 'Open' }));
    fireEvent.change(screen.getByLabelText('Core codes'), { target: { value: 'QQ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add Core codes' }));

    expect(state()).toMatchObject({
      campuses: ['NB'],
      core: { codes: ['QQ'], mode: 'ANY' },
      openStatuses: ['OPEN'],
      subjects: ['S299'],
      term: 'T2026F',
      text: 'data structures',
    });
    expect(screen.getByRole('button', { name: 'Clear Search text' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Clear Open status' })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: 'Clear Search text' }));
    expect(state().text).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'Clear filters' }));
    expect(state()).toEqual({ ...createNeutralFilterState('T2026F'), campuses: ['NB'] });
    expect(screen.getAllByLabelText('Search target preserved')).toHaveLength(2);
  });

  it('adds and removes availability windows, preserves the target, and submits once', () => {
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
    expect(screen.getAllByText('Friday 10:30–12:15')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: 'Remove availability window 1' }));
    expect(state().availability).toEqual([]);

    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(state()).toMatchObject({ term: 'T2026F', campuses: ['NB'] });
  });
});
