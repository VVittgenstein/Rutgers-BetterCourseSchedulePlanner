// @vitest-environment jsdom

import { cleanup, createEvent, fireEvent, render as renderLibrary, screen, within } from '@testing-library/react';
import type { ReactElement } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  CourseDetailView,
  CourseResultsView,
  SectionDetailView,
  SectionResultsView,
} from '../src/ui/shared/search/results';
import type {
  CatalogFieldKnowledge,
  CourseDetailResponseV1,
  CourseQueryResponseV1,
  NormalizedCourseVariantV1,
  NormalizedOccurrenceV1,
  NormalizedSectionV1,
  SectionDetailResponseV1,
  SectionQueryItemV1,
  SectionQueryResponseV1,
} from '../src/ui/shared/product';
import type { SupportedLocale } from '../src/ui/shared/i18n/contract';
import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';

const GROUP_KEY = { campus: 'NB', courseString: '01:198:211', term: '2026-9' } as const;
const SECTION_KEY = { campus: 'NB', index: '12345', term: '2026-9' } as const;
const SECOND_SECTION_KEY = { campus: 'NB', index: '54321', term: '2026-9' } as const;

function render(ui: ReactElement, locale: SupportedLocale = 'en-US') {
  return renderLibrary(<BcspI18nProvider initialLocale={locale}>{ui}</BcspI18nProvider>);
}

function known<T>(value: T): CatalogFieldKnowledge<T> {
  return { knowledge: 'KNOWN', presence: { presence: 'PRESENT', value } };
}

function variant(
  fingerprint: string,
  title: string,
  credits: string,
  supplement: string,
  sectionIndex: string,
): NormalizedCourseVariantV1 {
  return {
    key: { fingerprint, group: GROUP_KEY },
    title: known(title),
    expandedTitle: known(title),
    description: known(`${title} catalog description`),
    notes: known('Department note'),
    subjectGroupNotes: known('Group note'),
    subjectNotes: known('Subject note'),
    unitNotes: known('Unit note'),
    synopsisUrl: known('https://catalog.example.invalid/course'),
    prerequisiteNotes: known('Placement or prior programming course'),
    prerequisiteState: 'HAS',
    credits: known(credits),
    level: known('UNDERGRADUATE'),
    subjectCode: known('198'),
    subjectDescription: known('Computer Science'),
    courseNumber: known('211'),
    supplementCode: known(supplement),
    schoolCode: known('01'),
    offeringUnit: known('198'),
    offeringUnitTitle: known('Computer Science'),
    coreCodes: known(['QQ']),
    campusLocations: known(['NB']),
    sectionKeys: [{ ...SECTION_KEY, index: sectionIndex }],
  };
}

function normalizedSection(key = SECTION_KEY): NormalizedSectionV1 {
  return {
    key,
    variantKey: { fingerprint: 'lecture', group: GROUP_KEY },
    sectionNumber: known(key === SECTION_KEY ? '01' : '02'),
    subtitle: known('Data structures studio'),
    subtopic: known('Trees'),
    sectionNotes: known('Bring a laptop'),
    sessionDates: known('09/01–12/14'),
    sessionDatePrintIndicator: known('Y'),
    comments: known([]),
    commentsText: known('No additional comments'),
    crossListedSectionsText: known('None'),
    crossListedSectionType: known('NONE'),
    instructors: known(['Ada Lovelace']),
    instructorReliability: 'NAME_ONLY',
    rawSectionCourseType: known('LEC'),
    deliveryModality: 'ON_CAMPUS_OR_IN_PERSON',
    synchronicity: 'SYNC',
    examCode: known('A'),
    examCodeText: known('Common final'),
    specialPermissionAddCode: known('N'),
    specialPermissionAddDescription: known('No permission required'),
    specialPermissionDropCode: known('N'),
    specialPermissionDropDescription: known('No permission required'),
    majorCodes: known([]),
    unitCodes: known([]),
    minorCodes: known([]),
    honorProgramCodes: known([]),
    unitMajors: known([]),
    eligibilityText: known('Open to all students'),
    openToText: known('All students'),
    catalogOpenStatus: { provenance: 'CATALOG_SNAPSHOT_ONLY', value: known(true) },
    occurrenceKeys: known([{ ordinal: 1, section: key }]),
  };
}

function occurrence(key = SECTION_KEY): NormalizedOccurrenceV1 {
  return {
    key: { ordinal: 1, section: key },
    rawCode: known('LEC'),
    rawDescription: known('Lecture'),
    modality: known('ON_CAMPUS_OR_IN_PERSON'),
    synchronicity: known('SYNC'),
    rawDay: known('MW'),
    days: known(['MONDAY', 'WEDNESDAY']),
    rawStartTime: known('10:20 AM'),
    rawEndTime: known('11:40 AM'),
    time: { endMinute: 700, knowledge: 'KNOWN', startMinute: 620 },
    startDate: known('2026-09-01'),
    endDate: known('2026-12-14'),
    campus: known('NB'),
    campusName: known('New Brunswick'),
    building: known('HLL'),
    room: known('116'),
    requiredness: 'REQUIRED',
    kind: 'SCHEDULED',
    evidence: 'PHYSICAL',
    normalizationReason: 'NORMALIZED_MEETING',
  };
}

function sectionItem(
  outcome: 'MATCH' | 'UNCERTAIN' | 'NO_MATCH',
  key = SECTION_KEY,
): SectionQueryItemV1 {
  const reasons = outcome === 'UNCERTAIN'
    ? [{ code: 'UNKNOWN_VALUE', field: 'instructors' }] as const
    : [];
  return {
    section: normalizedSection(key),
    occurrences: [occurrence(key)],
    open: {
      freshUntil: '2026-07-15T03:10:00.000Z',
      observedAt: '2026-07-15T03:09:30.000Z',
      state: outcome === 'MATCH' ? 'OPEN' : 'UNKNOWN',
      uncertainty: outcome === 'MATCH' ? null : 'UNKNOWN_VALUE',
    },
    explanation: { outcome, reasons },
    filterMatches: [{
      explanation: { outcome, reasons },
      fieldId: 'FLT-S06',
    }],
  };
}

const FIRST_VARIANT = variant('lecture', 'Data Structures', '4.0', 'A', SECTION_KEY.index);
const SECOND_VARIANT = variant('honors', 'Data Structures Honors', '3.0', 'H', SECOND_SECTION_KEY.index);

const COURSE_RESPONSE: CourseQueryResponseV1 = {
  contractVersion: 1,
  page: { page: 1, pageSize: 25, total: 37, totalPages: 2 },
  items: [{
    explanation: { outcome: 'MATCH', reasons: [] },
    group: { key: GROUP_KEY, variantKeys: [FIRST_VARIANT.key, SECOND_VARIANT.key] },
    variants: [
      {
        explanation: { outcome: 'MATCH', reasons: [] },
        filterMatches: [],
        sections: [sectionItem('MATCH')],
        textMatch: { exactCourseIdentifier: true, matchedTokens: ['211'] },
        variant: FIRST_VARIANT,
      },
      {
        explanation: {
          outcome: 'UNCERTAIN',
          reasons: [{ code: 'UNKNOWN_VALUE', field: 'instructors' }],
        },
        filterMatches: [],
        sections: [sectionItem('UNCERTAIN', SECOND_SECTION_KEY)],
        textMatch: null,
        variant: SECOND_VARIANT,
      },
    ],
  }],
};

afterEach(cleanup);

function expandCourseSections(label = 'Show 1 Sections'): void {
  for (const disclosure of screen.getAllByText(label)) fireEvent.click(disclosure);
}

describe('typed search result and detail views', () => {
  it('formats Rutgers encoded credit values without exposing their storage representation', () => {
    const response: CourseQueryResponseV1 = {
      ...COURSE_RESPONSE,
      items: [{
        ...COURSE_RESPONSE.items[0]!,
        variants: [
          {
            ...COURSE_RESPONSE.items[0]!.variants[0]!,
            variant: { ...FIRST_VARIANT, credits: known('3_0') },
          },
          {
            ...COURSE_RESPONSE.items[0]!.variants[1]!,
            variant: { ...SECOND_VARIANT, credits: known('1_5') },
          },
        ],
      }],
    };
    render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={response}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    const variantFacts = screen.getAllByLabelText('Offering-defining fields');
    expect(within(variantFacts[0]!).getByText('3 credits')).toBeTruthy();
    expect(within(variantFacts[1]!).getByText('1.5 credits')).toBeTruthy();
    expect(screen.queryByText('3_0')).toBeNull();
    expect(screen.queryByText('1_5')).toBeNull();
  });

  it('leads with the course name, keeps the code as quiet meta, and never prints a fingerprint', () => {
    const view = render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={{
          ...COURSE_RESPONSE,
          items: [{
            ...COURSE_RESPONSE.items[0]!,
            group: { key: GROUP_KEY, variantKeys: [FIRST_VARIANT.key] },
            variants: [{
              ...COURSE_RESPONSE.items[0]!.variants[0]!,
              variant: {
                ...FIRST_VARIANT,
                expandedTitle: known('Introduction to Data Structures'),
                supplementCode: known(''),
              },
            }],
          }],
        }}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    // The expanded title is the more informative name, so it becomes the headline.
    const heading = screen.getByRole('heading', { name: 'Introduction to Data Structures' });
    expect(heading.tagName).toBe('H2');
    expect(heading.classList.contains('search-results__group-title')).toBe(true);
    const identity = heading.parentElement!.querySelector('.search-results__identity')!;
    expect(identity.textContent).toContain('01:198:211');
    expect(identity.textContent).toContain('4.0 credits');
    expect(identity.textContent).toContain('Computer Science');
    expect(identity.textContent).toContain('Undergraduate');
    expect(identity.textContent).toContain('Core QQ');
    // An empty supplement code never renders a label with nothing after it.
    expect(identity.textContent).not.toContain('Supplement');
    // A single offering is not disclosed as a variant set, and no hash reaches the DOM.
    expect(view.container.querySelector('.search-results__variant')).toBeNull();
    expect(view.container.innerHTML).not.toContain('lecture');
    expect(screen.queryByText(/MATCH/u)).toBeNull();
    expect(screen.getByRole('button', { name: 'Course detail' })).toBeTruthy();
  });

  it('discloses several genuine offerings by count and label instead of by fingerprint', () => {
    render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={COURSE_RESPONSE}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    expect(screen.getByText('2 offerings')).toBeTruthy();
    // Offering 1 carries the card's own name, so it shows its label rather than repeating it.
    expect(screen.getByRole('heading', { name: 'Offering 1' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Data Structures Honors' })).toBeTruthy();
    expect(screen.getAllByRole('heading', { name: 'Data Structures' }).length).toBe(1);
  });

  it('defensively removes NO_MATCH variants and Sections from search responses', () => {
    const hiddenTitle = 'Backend should not have returned this variant';
    const courseResponse: CourseQueryResponseV1 = {
      ...COURSE_RESPONSE,
      items: [{
        ...COURSE_RESPONSE.items[0]!,
        variants: [
          {
            ...COURSE_RESPONSE.items[0]!.variants[0]!,
            sections: [sectionItem('MATCH'), sectionItem('NO_MATCH', SECOND_SECTION_KEY)],
          },
          {
            ...COURSE_RESPONSE.items[0]!.variants[1]!,
            explanation: { outcome: 'NO_MATCH', reasons: [] },
            variant: { ...SECOND_VARIANT, title: known(hiddenTitle) },
          },
        ],
      }],
    };
    const courseView = render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={courseResponse}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    expect(screen.queryByRole('heading', { name: hiddenTitle })).toBeNull();
    expect(screen.getByText('Show 1 Sections')).toBeTruthy();
    fireEvent.click(screen.getByText('Show 1 Sections'));
    expect(courseView.container.querySelector('[data-section-index="12345"]')).not.toBeNull();
    expect(courseView.container.querySelector('[data-section-index="54321"]')).toBeNull();
    courseView.unmount();

    const sectionResponse: SectionQueryResponseV1 = {
      contractVersion: 1,
      items: [
        {
          courseFilterMatches: [],
          section: sectionItem('MATCH'),
          textMatch: null,
          variant: FIRST_VARIANT,
        },
        {
          courseFilterMatches: [],
          section: sectionItem('NO_MATCH', SECOND_SECTION_KEY),
          textMatch: null,
          variant: SECOND_VARIANT,
        },
      ],
      page: { page: 1, pageSize: 25, total: 2, totalPages: 1 },
    };
    const sectionView = render(
      <SectionResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={sectionResponse}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );
    expect(sectionView.container.querySelector('[data-section-index="12345"]')).not.toBeNull();
    expect(sectionView.container.querySelector('[data-section-index="54321"]')).toBeNull();
  });

  it('keeps Sections unmounted by default and exposes the complete cards through native keyboard disclosure', () => {
    const view = render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={{
          ...COURSE_RESPONSE,
          items: [{
            ...COURSE_RESPONSE.items[0]!,
            variants: [COURSE_RESPONSE.items[0]!.variants[0]!],
          }],
        }}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    const summary = screen.getByText('Show 1 Sections').closest('summary');
    const details = summary?.closest('details');
    expect(summary).not.toBeNull();
    expect(details?.open).toBe(false);
    expect(view.container.querySelector('[data-section-index="12345"]')).toBeNull();
    expect(screen.queryByText('Ada Lovelace')).toBeNull();

    summary?.focus();
    expect(document.activeElement).toBe(summary);
    fireEvent.keyDown(summary as HTMLElement, { key: 'Enter', code: 'Enter' });
    if (details === null || details === undefined) throw new Error('native details disclosure is required');
    details.open = true;
    fireEvent(details, new Event('toggle'));

    expect(screen.getByText('Hide 1 Sections')).toBeTruthy();
    const row = view.container.querySelector<HTMLElement>('[data-section-index="12345"]')!;
    expect(row).not.toBeNull();
    expect(row.querySelector('.search-results__section-title')?.textContent).toBe('12345');
    expect(within(row).getByText('Open').classList.contains('search-results__badge--open')).toBe(true);
    expect(screen.getByRole('link', { name: 'Open section' }).getAttribute('href')).toBe('/sections/12345');
    expect(within(row).getByText('Mon Wed · 10:20–11:40 · HLL 116')).toBeTruthy();
    expect(within(row).getByText('Section 01')).toBeTruthy();
    // A plain match prints no badge at all.
    expect(row.querySelector('.search-results__badge--match')).toBeNull();
    expect(row.querySelector('.search-results__note')).toBeNull();
  });

  it('accepts a session-controlled Section disclosure after the result view remounts', () => {
    const onSectionDisclosureChange = vi.fn();
    const response = {
      ...COURSE_RESPONSE,
      items: [{
        ...COURSE_RESPONSE.items[0]!,
        variants: [COURSE_RESPONSE.items[0]!.variants[0]!],
      }],
    };
    const view = render(
      <CourseResultsView
        expandedSectionDisclosures={new Set()}
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        onSectionDisclosureChange={onSectionDisclosureChange}
        response={response}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    fireEvent.click(screen.getByText('Show 1 Sections'));
    const [disclosureId] = onSectionDisclosureChange.mock.calls[0] ?? [];
    expect(disclosureId).toBe('2026-9\u0000NB\u000001:198:211\u0000lecture');
    expect(onSectionDisclosureChange).toHaveBeenCalledWith(disclosureId, true);

    view.rerender(
      <BcspI18nProvider initialLocale="en-US">
        <CourseResultsView
          expandedSectionDisclosures={new Set([disclosureId as string])}
          onCourseDetail={() => undefined}
          onPageChange={() => undefined}
          onSectionDisclosureChange={onSectionDisclosureChange}
          response={response}
          sectionHref={(key) => `/sections/${key.index}`}
        />
      </BcspI18nProvider>,
    );
    expect(screen.getByText('Hide 1 Sections')).toBeTruthy();
    expect(view.container.querySelector('[data-section-index="12345"]')).not.toBeNull();
  });

  it('keeps course offerings explicit and reports uncertainty as one quiet warn line', () => {
    const view = render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={COURSE_RESPONSE}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    expect(screen.getByRole('heading', { name: 'Data Structures' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Data Structures Honors' })).toBeTruthy();
    const variantFacts = screen.getAllByLabelText('Offering-defining fields');
    expect(within(variantFacts[0]!).getByText('4.0 credits')).toBeTruthy();
    expect(within(variantFacts[0]!).getByText('Supplement A')).toBeTruthy();
    expect(within(variantFacts[1]!).getByText('3.0 credits')).toBeTruthy();
    expect(within(variantFacts[1]!).getByText('Supplement H')).toBeTruthy();
    // Only doubt earns a badge; a plain match earns none.
    expect(screen.queryByText('MATCH')).toBeNull();
    expect(screen.getAllByText('Incomplete data').length).toBeGreaterThan(0);
    expect(view.container.querySelector('.search-results__badge--match')).toBeNull();
    expandCourseSections();
    expect(screen.getAllByText(/Mon Wed · 10:20–11:40 · HLL 116/u).length).toBe(2);

    const uncertainRow = view.container.querySelector<HTMLElement>('[data-section-index="54321"]')!;
    expect(uncertainRow.getAttribute('data-match-outcome')).toBe('UNCERTAIN');
    const note = uncertainRow.querySelector('.search-results__note--warn')!;
    expect(note.tagName).toBe('P');
    expect(note.textContent).toBe('The value is unknown.');
    expect(within(uncertainRow).getByText('Unknown')).toBeTruthy();
  });

  it('states a doubt every Section shares once above the list instead of on every row', () => {
    // Stale live seat evidence is a target-wide condition: the first seconds
    // after launch, or a failed Open fetch. Repeating it per row buries the list.
    const stale = (key: typeof SECTION_KEY): SectionQueryItemV1 => {
      const item = sectionItem('UNCERTAIN', key);
      return {
        ...item,
        open: { ...item.open, state: 'OPEN', uncertainty: 'SOURCE_UNAVAILABLE' },
        explanation: {
          outcome: 'UNCERTAIN',
          reasons: [{ code: 'SOURCE_UNAVAILABLE', field: 'FLT-S03' }],
        },
      };
    };
    const response: CourseQueryResponseV1 = {
      ...COURSE_RESPONSE,
      items: [{
        ...COURSE_RESPONSE.items[0]!,
        variants: COURSE_RESPONSE.items[0]!.variants.map((entry, index) => ({
          ...entry,
          explanation: { outcome: 'MATCH', reasons: [] },
          sections: [stale(index === 0 ? SECTION_KEY : SECOND_SECTION_KEY)],
        })),
      }],
    };

    const view = render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={response}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    const shared = screen.getByText(/Live seat data is not current/u);
    expect(shared.textContent).toContain('The source data is unavailable.');
    expandCourseSections();
    for (const index of ['12345', '54321']) {
      const row = view.container.querySelector<HTMLElement>(`[data-section-index="${index}"]`)!;
      expect(row.querySelector('.search-results__note--warn')).toBeNull();
      expect(within(row).queryByText('Incomplete data')).toBeNull();
    }
  });

  it('keeps per-row doubt when the Sections do not all share it', () => {
    // COURSE_RESPONSE mixes a certain Section with an uncertain one, so nothing
    // is hoisted and the uncertain row keeps its own note.
    const view = render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={COURSE_RESPONSE}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );
    expect(screen.queryByText(/Live seat data is not current/u)).toBeNull();
    expandCourseSections();
    const uncertainRow = view.container.querySelector<HTMLElement>('[data-section-index="54321"]')!;
    expect(uncertainRow.querySelector('.search-results__note--warn')).not.toBeNull();
  });

  it('translates Chinese result chrome while preserving Rutgers course and Section data', () => {
    render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={COURSE_RESPONSE}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
      'zh-CN',
    );

    expect(screen.getByRole('heading', { name: '课程结果' })).toBeTruthy();
    expect(screen.getByText('2 个开课版本')).toBeTruthy();
    expect(screen.getByRole('heading', { name: '版本 1' })).toBeTruthy();
    expandCourseSections('显示 1 个课节');
    expect(screen.getAllByRole('link', { name: '打开课节' }).length).toBeGreaterThan(0);
    expect(screen.getByRole('heading', { name: 'Data Structures' })).toBeTruthy();
    expect(screen.getByText('开放')).toBeTruthy();
    expect(screen.getAllByText(/周一 周三 · 10:20–11:40 · HLL 116/u).length).toBe(2);
    // No English chrome leaks into the Chinese UI.
    expect(screen.queryByText(/MATCH|UNCERTAIN|MONDAY/u)).toBeNull();
    expect(screen.getAllByText('数据不完整').length).toBeGreaterThan(0);
  });

  it('keeps a direct Section URL, delegates unmodified primary navigation, and pages with one-based callbacks', () => {
    const navigate = vi.fn();
    const page = vi.fn();
    render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={page}
        onSectionNavigate={navigate}
        response={COURSE_RESPONSE}
        sectionHref={(key) => `/catalog/${key.term}/${key.campus}/section/${key.index}`}
      />,
    );

    fireEvent.click(screen.getAllByText('Show 1 Sections')[0]!);
    const links = screen.getAllByRole('link', { name: 'Open section' });
    expect(links[0]?.getAttribute('href')).toBe('/catalog/2026-9/NB/section/12345');
    const click = createEvent.click(links[0]!, { button: 0 });
    fireEvent(links[0]!, click);
    expect(click.defaultPrevented).toBe(true);
    expect(navigate).toHaveBeenCalledWith(SECTION_KEY);
    fireEvent.click(screen.getByRole('button', { name: 'Next' }));
    expect(page).toHaveBeenCalledWith(2);
    expect(screen.getByRole('button', { name: 'Previous' }).hasAttribute('disabled')).toBe(true);
  });

  it.each([
    ['Ctrl', { button: 0, ctrlKey: true }],
    ['Command', { button: 0, metaKey: true }],
    ['Shift', { button: 0, shiftKey: true }],
    ['Alt', { altKey: true, button: 0 }],
    ['middle button', { button: 1 }],
  ])('leaves %s Section link activation to the browser', (_label, eventInit) => {
    const navigate = vi.fn();
    render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        onSectionNavigate={navigate}
        response={COURSE_RESPONSE}
        sectionHref={() => '#section'}
      />,
    );

    fireEvent.click(screen.getAllByText('Show 1 Sections')[0]!);
    const link = screen.getAllByRole('link', { name: 'Open section' })[0]!;
    const click = createEvent.click(link, eventInit);
    fireEvent(link, click);

    expect(click.defaultPrevented).toBe(false);
    expect(navigate).not.toHaveBeenCalled();
  });

  it('renders typed Section results and course navigation without flattening their variant', () => {
    const detail = vi.fn();
    const response: SectionQueryResponseV1 = {
      contractVersion: 1,
      items: [{
        courseFilterMatches: [],
        section: sectionItem('MATCH'),
        textMatch: null,
        variant: FIRST_VARIANT,
      }],
      page: { page: 1, pageSize: 25, total: 1, totalPages: 1 },
    };
    render(
      <SectionResultsView
        onCourseDetail={detail}
        onPageChange={() => undefined}
        response={response}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    const heading = screen.getByRole('heading', { name: 'Data Structures' });
    expect(heading.parentElement?.querySelector('.search-results__identity')?.textContent)
      .toContain('01:198:211');
    fireEvent.click(screen.getByRole('button', { name: 'Course detail' }));
    expect(detail).toHaveBeenCalledWith(GROUP_KEY);
  });

  it('shows reusable Course and Section detail fields from typed responses', () => {
    const courseDetail: CourseDetailResponseV1 = {
      contractVersion: 1,
      course: COURSE_RESPONSE.items[0]!,
    };
    const courseView = render(
      <CourseDetailView response={courseDetail} sectionHref={(key) => `/sections/${key.index}`} />,
    );
    expect(screen.getByText('Data Structures catalog description')).toBeTruthy();
    expect(screen.getAllByText('Placement or prior programming course').length).toBe(2);
    expect(screen.getAllByText(/Computer Science/u).length).toBeGreaterThan(0);
    courseView.unmount();

    const sectionDetail: SectionDetailResponseV1 = {
      contractVersion: 1,
      section: sectionItem('MATCH'),
      variant: FIRST_VARIANT,
    };
    render(
      <SectionDetailView response={sectionDetail} sectionHref={(key) => `/sections/${key.index}`} />,
    );
    expect(screen.getByText('Data structures studio')).toBeTruthy();
    expect(screen.getByText('Ada Lovelace')).toBeTruthy();
    expect(screen.getByText(/Common final/u)).toBeTruthy();
    expect(screen.getByText('No permission required')).toBeTruthy();
    expect(screen.getByText('No permission required').closest('.search-results__field')
      ?.classList.contains('search-results__field--full-row')).toBe(true);
  });
});
