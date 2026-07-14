// @vitest-environment jsdom

import { cleanup, createEvent, fireEvent, render, screen, within } from '@testing-library/react';
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

const GROUP_KEY = { campus: 'NB', courseString: '01:198:211', term: '2026-9' } as const;
const SECTION_KEY = { campus: 'NB', index: '12345', term: '2026-9' } as const;
const SECOND_SECTION_KEY = { campus: 'NB', index: '54321', term: '2026-9' } as const;

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
    expandedTitle: known(`${title} — expanded`),
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
  outcome: 'MATCH' | 'UNCERTAIN',
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

describe('typed search result and detail views', () => {
  it('keeps course variants explicit and exposes section evidence, live freshness, and uncertainty', () => {
    render(
      <CourseResultsView
        onCourseDetail={() => undefined}
        onPageChange={() => undefined}
        response={COURSE_RESPONSE}
        sectionHref={(key) => `/sections/${key.index}`}
      />,
    );

    expect(screen.getByRole('heading', { name: '01:198:211' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Data Structures' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: 'Data Structures Honors' })).toBeTruthy();
    const variantFacts = screen.getAllByLabelText('Variant-defining fields');
    expect(within(variantFacts[0]!).getByText('4.0')).toBeTruthy();
    expect(within(variantFacts[0]!).getByText('A')).toBeTruthy();
    expect(within(variantFacts[1]!).getByText('3.0')).toBeTruthy();
    expect(within(variantFacts[1]!).getByText('H')).toBeTruthy();
    expect(screen.getAllByText('MATCH').length).toBeGreaterThan(0);
    expect(screen.getAllByText('UNCERTAIN').length).toBeGreaterThan(0);
    expect(screen.getByText('Live OPEN')).toBeTruthy();
    expect(screen.getAllByText(/Fresh until 2026-07-15/u).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/MONDAY, WEDNESDAY · 10:20–11:40 · HLL 116/u).length).toBe(2);
    expect(screen.getByText('FLT-S06 · MATCH')).toBeTruthy();
    expect(screen.getByText('instructors: UNKNOWN_VALUE')).toBeTruthy();
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

    expect(screen.getByRole('heading', { name: /01:198:211 · Data Structures/u })).toBeTruthy();
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
  });
});
