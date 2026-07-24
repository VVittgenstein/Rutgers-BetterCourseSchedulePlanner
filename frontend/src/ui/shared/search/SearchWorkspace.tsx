import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from 'react';

import { ActionButton, StatePanel } from '../design-system';
import { isMessageKey } from '../i18n/contract';
import { filterSerializationIssueMessageKeys } from '../i18n/presenter';
import { useBcspI18n } from '../i18n/runtime';
import {
  FilterSerializationError,
  ProductClientError,
  coerceFilterStateV2,
  createCourseQueryRequestV1,
  createNeutralFilterState,
  isServiceStatusV2,
  isSearchDataReady,
  toFilterRequestV1,
  type CourseDetailResponseV1,
  type CourseGroupKey,
  type DynamicFilterInvalidValueV3,
  type FilterSerializationIssue,
  type FilterOptionsFieldV2,
  type FilterStateV1,
  type ProductRuntimePort,
  type SectionDetailResponseV1,
  type SectionKey,
  type ServiceStatus,
} from '../product';
import type { ShellDataState } from '../shell';
import { RouterLink, useAppRouter } from '../routing';
import { FilterPanel } from './filters';
import {
  SearchSessionProvider,
  type SearchScope,
  useOptionalSearchSession,
  useSearchSession,
} from './SearchSession';
import {
  QueryScopeControl,
  type QueryScopeUnavailableActionRenderer,
} from './QueryScopeControl';
import {
  CourseDetailView,
  CourseResultsView,
  SectionDetailView,
} from './results';
import { SearchWorkspaceStyles } from './searchStyles';
import { SearchControlPolishStyles } from './searchControlPolishStyles';

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

type ScopeValidationState =
  | 'PENDING'
  | 'ERROR'
  | { readonly invalidValues: readonly DynamicFilterInvalidValueV3[]; readonly kind: 'INVALID' }
  | null;

export interface SearchWorkspaceProps {
  readonly initialFilters?: FilterStateV1 | undefined;
  readonly onFiltersChange?: ((filters: FilterStateV1) => void) | undefined;
  readonly renderUnavailableScopeAction?: QueryScopeUnavailableActionRenderer | undefined;
  readonly runtime: ProductRuntimePort;
  readonly serviceStatus?: ServiceStatus | null | undefined;
  readonly shellState: Extract<ShellDataState, { status: 'READY' }>;
}

function initialTerm(
  _shellState: Extract<ShellDataState, { status: 'READY' }>,
  serviceStatus: ServiceStatus | null | undefined,
): string | null {
  if (serviceStatus !== null && serviceStatus !== undefined && isServiceStatusV2(serviceStatus)) {
    return serviceStatus.termWindow.currentTerm;
  }
  return null;
}

function createInitialFilters(
  shellState: Extract<ShellDataState, { status: 'READY' }>,
  serviceStatus: ServiceStatus | null | undefined,
  initialFilters?: FilterStateV1,
): FilterStateV1 {
  const term = initialTerm(shellState, serviceStatus);
  const filters = initialFilters === undefined
    ? createNeutralFilterState(term)
    : coerceFilterStateV2(initialFilters, term);
  return { ...filters, campuses: [], term };
}

function filtersForScope(filters: FilterStateV1, scope: SearchScope): FilterStateV1 {
  const changed = filters.term !== scope.term
    || filters.campuses.length !== scope.campuses.length
    || filters.campuses.some((campus, index) => campus !== scope.campuses[index]);
  if (!changed) return filters;
  return {
    ...filters,
    campuses: [...scope.campuses],
    term: scope.term,
  };
}

interface ScopeFilterRevalidation {
  readonly filters: FilterStateV1;
  readonly invalidValues: readonly DynamicFilterInvalidValueV3[];
}

async function revalidateScopeFilters(
  filters: FilterStateV1,
  scope: SearchScope,
  runtime: ProductRuntimePort,
  signal: AbortSignal,
): Promise<ScopeFilterRevalidation> {
  if (scope.term === null) throw new Error('A term is required before applying a scope.');
  const scoped = filtersForScope(filters, scope);
  const response = await runtime.product.validateDynamicFilters({
    filters: toFilterRequestV1(scoped),
  }, signal);
  return {
    filters: scoped,
    invalidValues: response.invalidValues,
  };
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
  if (kind === 'COURSES') return null;
  if (kind === 'IDLE') {
    return (
      <StatePanel
        detail={t('search.start_body')}
        heading={t('search.start_title')}
        kind="empty"
      />
    );
  }
  const compact = (state: 'loading' | 'empty' | 'error', heading: string, detail: string) => (
    <section
      aria-busy={state === 'loading' || undefined}
      aria-live={state === 'error' ? 'assertive' : 'polite'}
      className={`bcsp-search-state bcsp-search-state--${state}`}
      data-search-state={state}
      role={state === 'error' ? 'alert' : 'status'}
    >
      <span aria-hidden="true" className="bcsp-search-state__marker">
        {state === 'loading' ? '///' : state === 'error' ? '!' : 'Ø'}
      </span>
      <span className="bcsp-search-state__copy">
        <h4>{heading}</h4>
        <span>{detail}</span>
      </span>
    </section>
  );
  if (kind === 'LOADING') {
    return compact('loading', t('search.loading_title'), t('search.loading_body'));
  }
  if (kind === 'VALIDATION_ERROR') {
    return compact('error', t('search.validation_title'), message ?? t('search.validation_body'));
  }
  if (kind === 'ERROR') {
    return compact('error', t('search.error_title'), t('search.error_body'));
  }
  if (kind === 'NOT_READY') {
    return compact('loading', t('app.loading_title'), t('service.search_not_ready'));
  }
  return null;
}

function EmptySearchState() {
  const { t } = useBcspI18n();
  return (
    <section aria-live="polite" className="bcsp-search-state bcsp-search-state--empty" data-search-state="empty" role="status">
      <span aria-hidden="true" className="bcsp-search-state__marker">Ø</span>
      <span className="bcsp-search-state__copy">
        <h4>{t('search.empty_title')}</h4>
        <span>{t('search.empty_body')}</span>
      </span>
    </section>
  );
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
      { contractVersion: 3, key: sectionKey },
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
  renderUnavailableScopeAction,
  runtime,
  serviceStatus,
  shellState,
}: SearchWorkspaceProps) {
  const i18n = useBcspI18n();
  const filterFormId = useId();
  const { navigate, pathname } = useAppRouter();
  const session = useSearchSession();
  const directSection = useMemo(() => parseSectionRoute(pathname), [pathname]);
  const fallbackFilters = useMemo(
    () => createInitialFilters(shellState, serviceStatus, initialFilters),
    [initialFilters, serviceStatus, shellState],
  );
  const filters = session.state.draftFilters ?? fallbackFilters;
  const candidateScope = session.state.candidateScope ?? {
    campuses: [],
    term: fallbackFilters.term,
  };
  const appliedScope = session.state.appliedScope;
  const [query, setQuery] = useState<QueryState>({ kind: 'IDLE' });
  const [courseDetail, setCourseDetail] = useState<CourseDetailState>({ kind: 'CLOSED' });
  const searchAbort = useRef<AbortController | null>(null);
  const detailAbort = useRef<AbortController | null>(null);
  const scopeApplyAbort = useRef<AbortController | null>(null);
  const filtersRef = useRef<HTMLElement | null>(null);
  const resultsRef = useRef<HTMLElement | null>(null);
  const resultsHeadingRef = useRef<HTMLHeadingElement | null>(null);
  const pendingFocus = useRef<PendingSearchFocus>(null);
  const courseDetailReturnTarget = useRef<CourseDetailReturnTarget | null>(null);
  const externalFiltersSignature = useRef<string | null>(null);
  const observedInitialFilters = useRef(false);
  const [externalScopeRejected, setExternalScopeRejected] = useState(false);
  const [scopeValidation, setScopeValidation] = useState<ScopeValidationState>(null);
  const invalidateScopeBoundWork = useCallback(() => {
    searchAbort.current?.abort();
    searchAbort.current = null;
    detailAbort.current?.abort();
    detailAbort.current = null;
    scopeApplyAbort.current?.abort();
    scopeApplyAbort.current = null;
    pendingFocus.current = null;
    courseDetailReturnTarget.current = null;
  }, []);
  const searchDataReady = isSearchDataReady(serviceStatus, appliedScope ?? undefined);
  const searchAvailable = searchDataReady
    && appliedScope !== null;
  const previousSearchAvailable = useRef(searchAvailable);

  useEffect(() => () => invalidateScopeBoundWork(), [invalidateScopeBoundWork]);

  useLayoutEffect(() => {
    const filtersElement = filtersRef.current;
    if (filtersElement === null) return;
    filtersElement.scrollTop = session.restoreFilterScrollTop();
  }, [session.restoreFilterScrollTop]);

  useEffect(() => {
    session.initializeScope(
      { campuses: [], term: fallbackFilters.term },
      null,
      fallbackFilters,
    );
  }, [fallbackFilters, session.initializeScope]);

  useEffect(() => {
    const signature = initialFilters === undefined ? null : JSON.stringify(initialFilters);
    if (!observedInitialFilters.current) {
      observedInitialFilters.current = true;
      externalFiltersSignature.current = signature;
      return;
    }
    if (signature === externalFiltersSignature.current || initialFilters === undefined) return;
    if (serviceStatus === null || serviceStatus === undefined) return;
    externalFiltersSignature.current = signature;
    const next = coerceFilterStateV2(initialFilters, initialFilters.term);
    const scope = { campuses: next.campuses, term: next.term };
    if (!isSearchDataReady(serviceStatus, scope)) {
      setExternalScopeRejected(true);
      return;
    }
    if (JSON.stringify(next) === JSON.stringify(session.state.draftFilters)) return;
    invalidateScopeBoundWork();
    setExternalScopeRejected(false);
    session.applyScope(scope, next);
    setQuery({ kind: 'IDLE' });
    setCourseDetail({ kind: 'CLOSED' });
  }, [
    initialFilters,
    invalidateScopeBoundWork,
    serviceStatus,
    session.applyScope,
    session.state.draftFilters,
  ]);

  useEffect(() => {
    const becameAvailable = !previousSearchAvailable.current && searchAvailable;
    previousSearchAvailable.current = searchAvailable;
    if (query.kind === 'NOT_READY' && becameAvailable) setQuery({ kind: 'IDLE' });
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
      if (!abort.signal.aborted && searchAbort.current === abort) {
        session.recordSuccess(request, response);
        setQuery({ kind: 'COURSES' });
      }
    } catch (error) {
      if (abort.signal.aborted || searchAbort.current !== abort) return;
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
      { contractVersion: 3, key },
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
      contractVersion: 3,
      term: filters.term,
      campuses: filters.campuses,
      field,
      ...(optionQuery === undefined || optionQuery.trim() === '' ? {} : { query: optionQuery }),
      ...(field === 'COURSE_NUMBER_BAND' ? {} : { limit: 50 }),
    }, signal);
    return response ?? Promise.reject(new Error('Filter options are unavailable.'));
  }, [filters.campuses, filters.term, runtime]);

  const applyScope = useCallback((scope: SearchScope) => {
    scopeApplyAbort.current?.abort();
    const abort = new AbortController();
    scopeApplyAbort.current = abort;
    const scoped = filtersForScope(filters, scope);
    const targetBoundSelectionCount = scoped.subjects.length
      + scoped.keywords.length
      + scoped.courseNumberBands.length
      + scoped.levels.length
      + scoped.core.codes.length
      + scoped.instructors.length
      + scoped.meetingLocations.locations.length
      + scoped.examCodes.length;
    if (targetBoundSelectionCount === 0) {
      invalidateScopeBoundWork();
      session.applyScope(scope, scoped);
      onFiltersChange?.(scoped);
      setScopeValidation(null);
      setQuery({ kind: 'IDLE' });
      setCourseDetail({ kind: 'CLOSED' });
      return;
    }
    setScopeValidation('PENDING');
    void revalidateScopeFilters(filters, scope, runtime, abort.signal)
      .then(({ filters: next, invalidValues }) => {
        if (abort.signal.aborted) return;
        if (invalidValues.length > 0) {
          setScopeValidation({ invalidValues, kind: 'INVALID' });
          return;
        }
        invalidateScopeBoundWork();
        session.applyScope(scope, next);
        onFiltersChange?.(next);
        setScopeValidation(null);
        setQuery({ kind: 'IDLE' });
        setCourseDetail({ kind: 'CLOSED' });
      })
      .catch(() => {
        if (!abort.signal.aborted) setScopeValidation('ERROR');
      });
  }, [
    filters,
    invalidateScopeBoundWork,
    onFiltersChange,
    runtime,
    session.applyScope,
  ]);

  const changeCandidateScope = useCallback((scope: SearchScope) => {
    scopeApplyAbort.current?.abort();
    setScopeValidation(null);
    session.setCandidateScope(scope);
  }, [session.setCandidateScope]);

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
    <div
      className="bcsp-search-workspace"
      data-search-mode="courses"
      hidden={directSection !== null}
    >
      <SearchWorkspaceStyles />
      <section
        aria-labelledby="bcsp-search-filter-title"
        className="bcsp-search-workspace__filters"
        onKeyDown={(event) => {
          if (event.target !== event.currentTarget) return;
          const rail = event.currentTarget;
          if (rail.scrollHeight <= rail.clientHeight + 1) return;
          const pageStep = Math.max(44, Math.floor(rail.clientHeight * 0.9));
          if (event.key === 'Home') rail.scrollTop = 0;
          else if (event.key === 'End') rail.scrollTop = rail.scrollHeight;
          else if (event.key === 'PageDown') rail.scrollTop += pageStep;
          else if (event.key === 'PageUp') rail.scrollTop -= pageStep;
          else return;
          event.preventDefault();
        }}
        onScroll={(event) => session.saveFilterScrollTop(event.currentTarget.scrollTop)}
        ref={filtersRef}
        tabIndex={0}
      >
        <header className="bcsp-search-workspace__header">
          <div className="bcsp-search-workspace__titleline">
            <span className="bcsp-search-workspace__title-index" aria-hidden="true">01</span>
            <div>
              <p className="bcsp-search-workspace__title-kicker">RBCSP / QUERY</p>
              <h3 id="bcsp-search-filter-title">{i18n.t('search.filters_title')}</h3>
            </div>
          </div>
          <p>{i18n.t('search.course_intro')}</p>
        </header>
        <div className="bcsp-search-workspace__scope">
          {externalScopeRejected ? (
            <p className="bcsp-search-workspace__scope-error" role="alert">
              {i18n.t('scope.external_definition_unavailable')}
            </p>
          ) : null}
          {typeof scopeValidation === 'object' && scopeValidation?.kind === 'INVALID' ? (
            <p className="bcsp-search-workspace__scope-error" role="alert">
              {i18n.t('scope.invalid_options_blocked', {
                values: scopeValidation.invalidValues.map(({ field, value }) => {
                  const definition = shellState.filterSchema.fields.find(({ stableId }) => stableId === field);
                  const label = definition !== undefined && isMessageKey(definition.i18nKey)
                    ? i18n.t(definition.i18nKey)
                    : i18n.t('filter.form_label');
                  return `${label}: ${value}`;
                }).join(', '),
              })}
            </p>
          ) : null}
          {scopeValidation === 'ERROR' ? (
            <p className="bcsp-search-workspace__scope-error" role="alert">
              {i18n.t('scope.validation_failed')}
            </p>
          ) : null}
          <QueryScopeControl
            actionPending={scopeValidation === 'PENDING'}
            applied={appliedScope}
            candidate={candidateScope}
            discovery={shellState.discovery}
            onApply={applyScope}
            onCandidateChange={changeCandidateScope}
            renderUnavailableAction={renderUnavailableScopeAction}
            searchAvailable={searchAvailable}
            searchFormId={filterFormId}
            searchPending={query.kind === 'LOADING'}
            status={serviceStatus ?? null}
          />
        </div>
        <FilterPanel
          disabled={query.kind === 'LOADING' || !searchAvailable}
          discovery={shellState.discovery}
          formId={filterFormId}
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
      <SearchControlPolishStyles />
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
