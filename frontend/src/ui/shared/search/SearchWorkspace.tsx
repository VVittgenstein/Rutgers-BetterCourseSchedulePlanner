import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';

import { ActionButton, StatePanel } from '../design-system';
import { filterSerializationIssueMessageKeys } from '../i18n/presenter';
import { useBcspI18n } from '../i18n/runtime';
import {
  FilterSerializationError,
  ProductClientError,
  coerceFilterStateV2,
  createCourseQueryRequestV1,
  createNeutralFilterState,
  isSearchDataReady,
  type CourseDetailResponseV1,
  type CourseGroupKey,
  type FilterSerializationIssue,
  type FilterOptionsFieldV2,
  type FilterStateV1,
  type ProductRuntimePort,
  type SectionDetailResponseV1,
  type SectionKey,
  type ServiceStatusV1,
} from '../product';
import type { ShellDataState } from '../shell';
import { RouterLink, useAppRouter } from '../routing';
import { FilterPanel } from './filters';
import {
  SearchSessionProvider,
  useOptionalSearchSession,
  useSearchSession,
} from './SearchSession';
import {
  CourseDetailView,
  CourseResultsView,
  SectionDetailView,
} from './results';
import { SearchWorkspaceStyles } from './searchStyles';

type QueryState =
  | { readonly kind: 'IDLE' }
  | { readonly kind: 'LOADING' }
  | {
    readonly kind: 'VALIDATION_ERROR';
    readonly issue: FilterSerializationIssue;
  }
  | { readonly kind: 'ERROR' }
  | { readonly kind: 'NOT_READY' }
  | { readonly kind: 'COURSES' };

type CourseDetailState =
  | { readonly kind: 'CLOSED' }
  | { readonly kind: 'LOADING' }
  | { readonly kind: 'ERROR' }
  | { readonly kind: 'READY'; readonly response: CourseDetailResponseV1 };

type SectionDetailState =
  | { readonly kind: 'LOADING' }
  | { readonly kind: 'ERROR' }
  | { readonly kind: 'READY'; readonly response: SectionDetailResponseV1 };

interface CourseDetailReturnTarget {
  readonly buttonIndex: number;
  readonly element: HTMLElement | null;
  readonly key: CourseGroupKey;
}

type PendingSearchFocus = 'OUTPUT' | 'COURSE_TRIGGER' | null;

export interface SearchWorkspaceProps {
  readonly initialFilters?: FilterStateV1 | undefined;
  readonly onFiltersChange?: ((filters: FilterStateV1) => void) | undefined;
  readonly runtime: ProductRuntimePort;
  readonly serviceStatus?: ServiceStatusV1 | null | undefined;
  readonly shellState: Extract<ShellDataState, { status: 'READY' }>;
}

interface RutgersTermRank {
  readonly termCode: number;
  readonly year: number;
}

function rutgersTermRank(term: string): RutgersTermRank | null {
  const match = /^([0179])(\d{4})$/u.exec(term);
  const termCodeText = match?.[1];
  const yearText = match?.[2];
  if (termCodeText === undefined || yearText === undefined) return null;
  return { termCode: Number(termCodeText), year: Number(yearText) };
}

function initialTarget(
  shellState: Extract<ShellDataState, { status: 'READY' }>,
) {
  const targets = shellState.discovery.targets;
  const targetsWithSubjects = new Set(
    shellState.discovery.subjects.map((subject) =>
      `${subject.target.term}\u0000${subject.target.campus}`),
  );
  let selected = targets[0];
  let selectedRank: RutgersTermRank | null = null;
  let selectedHasSubjects = selected === undefined
    ? false
    : targetsWithSubjects.has(`${selected.key.term}\u0000${selected.key.campus}`);
  for (const target of targets) {
    const rank = rutgersTermRank(target.key.term);
    if (rank === null) continue;
    const hasSubjects = targetsWithSubjects.has(
      `${target.key.term}\u0000${target.key.campus}`,
    );
    if (
      selectedRank === null
      || rank.year > selectedRank.year
      || (rank.year === selectedRank.year && rank.termCode > selectedRank.termCode)
      || (
        rank.year === selectedRank.year
        && rank.termCode === selectedRank.termCode
        && hasSubjects
        && !selectedHasSubjects
      )
    ) {
      selected = target;
      selectedRank = rank;
      selectedHasSubjects = hasSubjects;
    }
  }
  return selected;
}

function createInitialFilters(
  shellState: Extract<ShellDataState, { status: 'READY' }>,
): FilterStateV1 {
  const target = initialTarget(shellState);
  const filters = createNeutralFilterState(target?.key.term ?? null);
  return target === undefined ? filters : { ...filters, campuses: [target.key.campus] };
}

function sectionHref(key: SectionKey): string {
  return `/sections/${encodeURIComponent(key.term)}/${encodeURIComponent(key.campus)}/${encodeURIComponent(key.index)}`;
}

function parseSectionRoute(pathname: string): SectionKey | null | 'INVALID' {
  if (!pathname.startsWith('/sections/')) return null;
  const segments = pathname.split('/');
  if (segments.length !== 5 || segments[0] !== '' || segments[1] !== 'sections') return 'INVALID';
  try {
    const [term, campus, index] = segments.slice(2).map((segment) => decodeURIComponent(segment));
    const safeIdentity = /^[A-Za-z0-9_-]{1,64}$/u;
    if (
      term === undefined
      || campus === undefined
      || index === undefined
      || !safeIdentity.test(term)
      || !safeIdentity.test(campus)
      || !/^\d{5}$/u.test(index)
    ) {
      return 'INVALID';
    }
    return { term, campus, index };
  } catch {
    return 'INVALID';
  }
}

function SearchState({
  kind,
  message,
}: {
  readonly kind: QueryState['kind'];
  readonly message?: string | undefined;
}) {
  const { t } = useBcspI18n();
  if (kind === 'LOADING') {
    return <StatePanel detail={t('search.loading_body')} heading={t('search.loading_title')} kind="loading" />;
  }
  if (kind === 'VALIDATION_ERROR') {
    return (
      <StatePanel
        detail={message ?? t('search.validation_body')}
        heading={t('search.validation_title')}
        kind="error"
      />
    );
  }
  if (kind === 'ERROR') {
    return <StatePanel detail={t('search.error_body')} heading={t('search.error_title')} kind="error" />;
  }
  if (kind === 'NOT_READY') {
    return <StatePanel detail={t('service.search_not_ready')} heading={t('app.loading_title')} kind="loading" />;
  }
  return <StatePanel detail={t('search.start_body')} heading={t('search.start_title')} kind="empty" />;
}

function EmptySearchState() {
  const { t } = useBcspI18n();
  return <StatePanel detail={t('search.empty_body')} heading={t('search.empty_title')} kind="empty" />;
}

function DirectSectionRoute({
  runtime,
  sectionKey,
}: {
  readonly runtime: ProductRuntimePort;
  readonly sectionKey: SectionKey | 'INVALID';
}) {
  const { t } = useBcspI18n();
  const { navigate } = useAppRouter();
  const [state, setState] = useState<SectionDetailState>(() =>
    sectionKey === 'INVALID' ? { kind: 'ERROR' } : { kind: 'LOADING' },
  );
  const identity = sectionKey === 'INVALID'
    ? sectionKey
    : `${sectionKey.term}\u0000${sectionKey.campus}\u0000${sectionKey.index}`;

  useEffect(() => {
    if (sectionKey === 'INVALID') {
      setState({ kind: 'ERROR' });
      return undefined;
    }
    const abort = new AbortController();
    setState({ kind: 'LOADING' });
    void runtime.product.sectionDetail(
      { contractVersion: 2, key: sectionKey },
      abort.signal,
    ).then((response) => {
      setState({ kind: 'READY', response });
    }).catch(() => {
      if (!abort.signal.aborted) setState({ kind: 'ERROR' });
    });
    return () => abort.abort();
  }, [identity, runtime]);

  return (
    <div className="bcsp-search-workspace" data-detail-route="true">
      <SearchWorkspaceStyles />
      <section className="bcsp-search-workspace__results">
        <div className="bcsp-search-workspace__detail-actions">
          <RouterLink className="bcsp-search-workspace__back" to="/">
            &lt;&lt; {t('search.back_to_courses')}
          </RouterLink>
          <p className="bcsp-search-workspace__route-meta">/sections/term/campus/index</p>
        </div>
        {state.kind === 'LOADING' ? (
          <div className="bcsp-search-workspace__state">
            <StatePanel detail={t('search.loading_body')} heading={t('search.section_detail_title')} kind="loading" />
          </div>
        ) : state.kind === 'ERROR' ? (
          <div className="bcsp-search-workspace__state">
            <StatePanel
              action={<ActionButton onClick={() => navigate('/')} tone="accent">{t('action.back')}</ActionButton>}
              detail={t('search.direct_route_error')}
              heading={t('search.section_detail_title')}
              kind="error"
            />
          </div>
        ) : (
          <SectionDetailView
            onSectionNavigate={(key) => navigate(sectionHref(key))}
            response={state.response}
            sectionHref={sectionHref}
          />
        )}
      </section>
    </div>
  );
}

function SearchWorkspaceController({
  initialFilters,
  onFiltersChange,
  runtime,
  serviceStatus,
  shellState,
}: SearchWorkspaceProps) {
  const i18n = useBcspI18n();
  const { navigate, pathname } = useAppRouter();
  const session = useSearchSession();
  const directSection = useMemo(() => parseSectionRoute(pathname), [pathname]);
  const fallbackFilters = useMemo(() =>
    initialFilters === undefined
      ? createInitialFilters(shellState)
      : coerceFilterStateV2(initialFilters, createInitialFilters(shellState).term),
  [initialFilters, shellState]);
  const filters = session.state.draftFilters ?? fallbackFilters;
  const [query, setQuery] = useState<QueryState>({ kind: 'IDLE' });
  const [courseDetail, setCourseDetail] = useState<CourseDetailState>({ kind: 'CLOSED' });
  const searchAbort = useRef<AbortController | null>(null);
  const detailAbort = useRef<AbortController | null>(null);
  const filtersRef = useRef<HTMLElement | null>(null);
  const resultsRef = useRef<HTMLElement | null>(null);
  const resultsHeadingRef = useRef<HTMLHeadingElement | null>(null);
  const pendingFocus = useRef<PendingSearchFocus>(null);
  const courseDetailReturnTarget = useRef<CourseDetailReturnTarget | null>(null);
  const searchDataReady = isSearchDataReady(serviceStatus);
  const searchAvailable = searchDataReady
    && filters.term !== null
    && filters.campuses.length > 0;

  useLayoutEffect(() => {
    if (filtersRef.current !== null) {
      filtersRef.current.scrollTop = session.restoreFilterScrollTop();
    }
  }, [session.restoreFilterScrollTop]);

  useEffect(() => () => {
    searchAbort.current?.abort();
    detailAbort.current?.abort();
  }, []);

  useEffect(() => {
    if (session.state.draftFilters === null) {
      session.setDraftFilters(fallbackFilters, false);
      return;
    }
    if (initialFilters !== undefined || session.state.draftWasEdited) return;
    const next = createInitialFilters(shellState);
    if (
      filters.term === next.term
      && filters.campuses.length === next.campuses.length
      && filters.campuses.every((campus, index) => campus === next.campuses[index])
    ) {
      return;
    }
    searchAbort.current?.abort();
    detailAbort.current?.abort();
    pendingFocus.current = null;
    courseDetailReturnTarget.current = null;
    session.setDraftFilters(next, false);
    setQuery({ kind: 'IDLE' });
    setCourseDetail({ kind: 'CLOSED' });
    onFiltersChange?.(next);
  }, [
    fallbackFilters,
    filters,
    initialFilters,
    onFiltersChange,
    session.setDraftFilters,
    session.state.draftFilters,
    session.state.draftWasEdited,
    shellState,
  ]);

  useEffect(() => {
    if (query.kind === 'NOT_READY' && searchAvailable) setQuery({ kind: 'IDLE' });
  }, [query.kind, searchAvailable]);

  useEffect(() => {
    if (pendingFocus.current === null) return;
    if (query.kind === 'LOADING' || courseDetail.kind === 'LOADING') return;
    if (query.kind === 'VALIDATION_ERROR') {
      pendingFocus.current = null;
      return;
    }
    const results = resultsRef.current;
    if (results === null) return;
    if (pendingFocus.current === 'COURSE_TRIGGER') {
      const returnTarget = courseDetailReturnTarget.current;
      const resultButtons = [...results.querySelectorAll<HTMLElement>('.search-results__button')];
      const matchingGroup = returnTarget === null
        ? undefined
        : [...results.querySelectorAll<HTMLElement>('[data-course-group]')]
          .find((group) => group.dataset.courseGroup === returnTarget.key.courseString);
      const restored = returnTarget?.element?.isConnected === true
        ? returnTarget.element
        : matchingGroup?.querySelector<HTMLElement>('.search-results__button')
          ?? resultButtons[returnTarget?.buttonIndex ?? -1]
          ?? null;
      pendingFocus.current = null;
      if (restored !== null) {
        restored.focus({ preventScroll: true });
        restored.scrollIntoView?.({ behavior: 'auto', block: 'nearest' });
        return;
      }
    }
    pendingFocus.current = null;
    resultsHeadingRef.current?.focus({ preventScroll: true });
    results.scrollIntoView?.({ behavior: 'auto', block: 'start' });
  }, [courseDetail.kind, query.kind]);

  const runSearch = useCallback(async (page = 1, fromSuccessfulRequest = false) => {
    if (!searchDataReady || (!fromSuccessfulRequest && !searchAvailable)) {
      setQuery({ kind: 'NOT_READY' });
      return;
    }
    searchAbort.current?.abort();
    detailAbort.current?.abort();
    const abort = new AbortController();
    searchAbort.current = abort;
    pendingFocus.current = 'OUTPUT';
    setCourseDetail({ kind: 'CLOSED' });
    setQuery({ kind: 'LOADING' });
    try {
      const successfulRequest = session.state.lastSuccessfulRequest;
      const request = fromSuccessfulRequest && successfulRequest !== null
        ? {
          ...successfulRequest,
          page: { ...successfulRequest.page, page },
          sort: { ...successfulRequest.sort },
        }
        : createCourseQueryRequestV1(filters, { page, pageSize: 25 });
      session.recordSubmission(request);
      const response = await runtime.product.searchCourses(request, abort.signal);
      if (!abort.signal.aborted) {
        session.recordSuccess(request, response);
        setQuery({ kind: 'COURSES' });
      }
    } catch (error) {
      if (abort.signal.aborted) return;
      setQuery(error instanceof FilterSerializationError
        ? { kind: 'VALIDATION_ERROR', issue: error.issue }
        : error instanceof ProductClientError && (
          error.apiError?.error.code === 'CATALOG_NOT_READY'
          || error.apiError?.error.code === 'SEARCH_DATA_NOT_READY'
        )
          ? { kind: 'NOT_READY' }
          : { kind: 'ERROR' });
    }
  }, [
    filters,
    runtime,
    searchAvailable,
    searchDataReady,
    session.recordSubmission,
    session.recordSuccess,
    session.state.lastSuccessfulRequest,
  ]);

  const openCourseDetail = useCallback((key: CourseGroupKey) => {
    detailAbort.current?.abort();
    const abort = new AbortController();
    detailAbort.current = abort;
    const results = resultsRef.current;
    const activeElement = globalThis.document?.activeElement;
    const resultButtons = results === null
      ? []
      : [...results.querySelectorAll<HTMLElement>('.search-results__button')];
    const trigger = activeElement instanceof HTMLElement && results?.contains(activeElement) === true
      ? activeElement
      : null;
    courseDetailReturnTarget.current = {
      buttonIndex: trigger === null ? -1 : resultButtons.indexOf(trigger),
      element: trigger,
      key,
    };
    pendingFocus.current = 'OUTPUT';
    setCourseDetail({ kind: 'LOADING' });
    void runtime.product.courseDetail(
      { contractVersion: 2, key },
      abort.signal,
    ).then((response) => {
      if (!abort.signal.aborted) setCourseDetail({ kind: 'READY', response });
    }).catch(() => {
      if (!abort.signal.aborted) setCourseDetail({ kind: 'ERROR' });
    });
  }, [runtime]);

  const loadFilterOptions = useCallback((
    field: FilterOptionsFieldV2,
    optionQuery?: string,
    signal?: AbortSignal,
  ) => {
    if (filters.term === null || filters.campuses.length === 0) {
      return Promise.reject(new Error('Filter options require a selected target.'));
    }
    const response = runtime.product.filterOptions?.({
      contractVersion: 2,
      term: filters.term,
      campuses: filters.campuses,
      field,
      ...(optionQuery === undefined || optionQuery.trim() === '' ? {} : { query: optionQuery }),
      limit: 50,
    }, signal);
    return response ?? Promise.reject(new Error('Filter options are unavailable.'));
  }, [filters.campuses, filters.term, runtime]);

  const retainedResponse = session.state.lastSuccessfulResponse;
  const empty = retainedResponse !== null && retainedResponse.items.length === 0;
  const retainedFeedback = query.kind === 'ERROR'
    || query.kind === 'NOT_READY'
    || query.kind === 'VALIDATION_ERROR'
    ? (
      <SearchState
        kind={query.kind}
        message={query.kind === 'VALIDATION_ERROR'
          ? i18n.t(filterSerializationIssueMessageKeys[query.issue])
          : undefined}
      />
    )
    : null;

  let results;
  if (courseDetail.kind === 'LOADING') {
    results = <SearchState kind="LOADING" />;
  } else if (courseDetail.kind === 'ERROR') {
    results = <SearchState kind="ERROR" />;
  } else if (courseDetail.kind === 'READY') {
    results = (
      <>
        <div className="bcsp-search-workspace__detail-actions">
          <ActionButton onClick={() => {
            pendingFocus.current = 'COURSE_TRIGGER';
            setCourseDetail({ kind: 'CLOSED' });
          }} tone="quiet">
            &lt;&lt; {i18n.t('action.back')}
          </ActionButton>
          <p className="bcsp-search-workspace__route-meta">{i18n.t('search.course_detail_title')}</p>
        </div>
        <CourseDetailView
          expandedSectionDisclosures={session.state.expandedSectionDisclosures}
          onSectionDisclosureChange={session.setSectionDisclosureExpanded}
          onSectionNavigate={(key) => navigate(sectionHref(key))}
          response={courseDetail.response}
          sectionHref={sectionHref}
        />
      </>
    );
  } else if (retainedResponse === null) {
    results = (
      <SearchState
        kind={query.kind}
        message={query.kind === 'VALIDATION_ERROR'
          ? i18n.t(filterSerializationIssueMessageKeys[query.issue])
          : undefined}
      />
    );
  } else if (empty) {
    results = <>{retainedFeedback}<EmptySearchState /></>;
  } else {
    results = (
      <>
        {retainedFeedback}
        <CourseResultsView
          expandedSectionDisclosures={session.state.expandedSectionDisclosures}
          onCourseDetail={openCourseDetail}
          onPageChange={(page) => void runSearch(page, true)}
          onSectionDisclosureChange={session.setSectionDisclosureExpanded}
          onSectionNavigate={(key) => navigate(sectionHref(key))}
          response={retainedResponse}
          sectionHref={sectionHref}
        />
      </>
    );
  }

  return (
    <>
    {directSection === null ? null : <DirectSectionRoute runtime={runtime} sectionKey={directSection} />}
    <div className="bcsp-search-workspace" data-search-mode="courses" hidden={directSection !== null}>
      <SearchWorkspaceStyles />
      <section
        aria-labelledby="bcsp-search-filter-title"
        className="bcsp-search-workspace__filters"
        onScroll={(event) => session.saveFilterScrollTop(event.currentTarget.scrollTop)}
        ref={filtersRef}
      >
        <header className="bcsp-search-workspace__header">
          <h3 id="bcsp-search-filter-title">{i18n.t('search.filters_title')}</h3>
          <p>{i18n.t('search.course_intro')}</p>
        </header>
        <FilterPanel
          disabled={query.kind === 'LOADING' || !searchAvailable}
          discovery={shellState.discovery}
          loadOptions={loadFilterOptions}
          onChange={(next) => {
            session.setDraftFilters(next, true);
            onFiltersChange?.(next);
            setQuery({ kind: 'IDLE' });
            setCourseDetail({ kind: 'CLOSED' });
          }}
          onSubmit={() => void runSearch(1)}
          searchAvailable={searchAvailable}
          schema={shellState.filterSchema}
          validationIssue={query.kind === 'VALIDATION_ERROR'
            ? {
              issue: query.issue,
              message: i18n.t(filterSerializationIssueMessageKeys[query.issue]),
            }
            : undefined}
          value={filters}
        />
      </section>
      <section
        aria-labelledby="bcsp-search-results-title"
        className="bcsp-search-workspace__results"
        ref={resultsRef}
      >
        <h3
          className="bcsp-visually-hidden"
          id="bcsp-search-results-title"
          ref={resultsHeadingRef}
          tabIndex={-1}
        >
          {i18n.t('search.results_title')}
        </h3>
        <div className="bcsp-search-workspace__state" data-query-state={query.kind.toLowerCase()}>
          {results}
        </div>
      </section>
    </div>
    </>
  );
}

export function SearchWorkspace(props: SearchWorkspaceProps) {
  const existingSession = useOptionalSearchSession();
  if (existingSession !== null) return <SearchWorkspaceController {...props} />;
  return (
    <SearchSessionProvider>
      <SearchWorkspaceController {...props} />
    </SearchSessionProvider>
  );
}
