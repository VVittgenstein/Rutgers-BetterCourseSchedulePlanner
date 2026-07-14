import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import { ActionButton, StatePanel } from '../design-system';
import { filterSerializationIssueMessageKeys } from '../i18n/presenter';
import { useBcspI18n } from '../i18n/runtime';
import {
  FilterSerializationError,
  createCourseQueryRequestV1,
  createNeutralFilterState,
  createSectionQueryRequestV1,
  type CourseDetailResponseV1,
  type CourseGroupKey,
  type CourseQueryResponseV1,
  type FilterSerializationIssue,
  type FilterStateV1,
  type ProductRuntimePort,
  type SectionDetailResponseV1,
  type SectionKey,
  type SectionQueryResponseV1,
} from '../product';
import type { ShellDataState } from '../shell';
import { RouterLink, useAppRouter } from '../routing';
import { FilterPanel } from './filters';
import {
  CourseDetailView,
  CourseResultsView,
  SectionDetailView,
  SectionResultsView,
} from './results';
import { SearchWorkspaceStyles } from './searchStyles';

type SearchMode = 'COURSES' | 'SECTIONS';

type QueryState =
  | { readonly kind: 'IDLE' }
  | { readonly kind: 'LOADING' }
  | {
    readonly kind: 'VALIDATION_ERROR';
    readonly issue: FilterSerializationIssue;
  }
  | { readonly kind: 'ERROR' }
  | { readonly kind: 'COURSES'; readonly response: CourseQueryResponseV1 }
  | { readonly kind: 'SECTIONS'; readonly response: SectionQueryResponseV1 };

type CourseDetailState =
  | { readonly kind: 'CLOSED' }
  | { readonly kind: 'LOADING' }
  | { readonly kind: 'ERROR' }
  | { readonly kind: 'READY'; readonly response: CourseDetailResponseV1 };

type SectionDetailState =
  | { readonly kind: 'LOADING' }
  | { readonly kind: 'ERROR' }
  | { readonly kind: 'READY'; readonly response: SectionDetailResponseV1 };

export interface SearchWorkspaceProps {
  readonly initialFilters?: FilterStateV1 | undefined;
  readonly onFiltersChange?: ((filters: FilterStateV1) => void) | undefined;
  readonly runtime: ProductRuntimePort;
  readonly shellState: Extract<ShellDataState, { status: 'READY' }>;
}

function createInitialFilters(
  shellState: Extract<ShellDataState, { status: 'READY' }>,
): FilterStateV1 {
  const target = shellState.discovery.targets[0];
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
      { contractVersion: 1, key: sectionKey },
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
          <RouterLink className="bcsp-search-workspace__back" to="/sections">
            &lt;&lt; {t('search.back_to_sections')}
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
              action={<ActionButton onClick={() => navigate('/sections')} tone="accent">{t('action.back')}</ActionButton>}
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

export function SearchWorkspace({
  initialFilters,
  onFiltersChange,
  runtime,
  shellState,
}: SearchWorkspaceProps) {
  const i18n = useBcspI18n();
  const { navigate, pathname } = useAppRouter();
  const directSection = useMemo(() => parseSectionRoute(pathname), [pathname]);
  const mode: SearchMode = pathname === '/sections' || directSection !== null
    ? 'SECTIONS'
    : 'COURSES';
  const [filters, setFilters] = useState<FilterStateV1>(() =>
    initialFilters ?? createInitialFilters(shellState));
  const [query, setQuery] = useState<QueryState>({ kind: 'IDLE' });
  const [courseDetail, setCourseDetail] = useState<CourseDetailState>({ kind: 'CLOSED' });
  const searchAbort = useRef<AbortController | null>(null);
  const detailAbort = useRef<AbortController | null>(null);

  useEffect(() => () => {
    searchAbort.current?.abort();
    detailAbort.current?.abort();
  }, []);

  useEffect(() => {
    if (directSection !== null) return;
    searchAbort.current?.abort();
    detailAbort.current?.abort();
    setQuery({ kind: 'IDLE' });
    setCourseDetail({ kind: 'CLOSED' });
  }, [mode, directSection]);

  const runSearch = useCallback(async (page = 1) => {
    searchAbort.current?.abort();
    detailAbort.current?.abort();
    const abort = new AbortController();
    searchAbort.current = abort;
    setCourseDetail({ kind: 'CLOSED' });
    setQuery({ kind: 'LOADING' });
    try {
      if (mode === 'COURSES') {
        const response = await runtime.product.searchCourses(
          createCourseQueryRequestV1(filters, { page, pageSize: 25 }),
          abort.signal,
        );
        if (!abort.signal.aborted) setQuery({ kind: 'COURSES', response });
      } else {
        const response = await runtime.product.searchSections(
          createSectionQueryRequestV1(filters, { page, pageSize: 25 }),
          abort.signal,
        );
        if (!abort.signal.aborted) setQuery({ kind: 'SECTIONS', response });
      }
    } catch (error) {
      if (abort.signal.aborted) return;
      setQuery(error instanceof FilterSerializationError
        ? { kind: 'VALIDATION_ERROR', issue: error.issue }
        : { kind: 'ERROR' });
    }
  }, [filters, mode, runtime]);

  const openCourseDetail = useCallback((key: CourseGroupKey) => {
    detailAbort.current?.abort();
    const abort = new AbortController();
    detailAbort.current = abort;
    setCourseDetail({ kind: 'LOADING' });
    void runtime.product.courseDetail(
      { contractVersion: 1, key },
      abort.signal,
    ).then((response) => {
      if (!abort.signal.aborted) setCourseDetail({ kind: 'READY', response });
    }).catch(() => {
      if (!abort.signal.aborted) setCourseDetail({ kind: 'ERROR' });
    });
  }, [runtime]);

  if (directSection !== null) {
    return <DirectSectionRoute runtime={runtime} sectionKey={directSection} />;
  }

  const empty = (query.kind === 'COURSES' || query.kind === 'SECTIONS')
    && query.response.items.length === 0;

  let results;
  if (courseDetail.kind === 'LOADING') {
    results = <SearchState kind="LOADING" />;
  } else if (courseDetail.kind === 'ERROR') {
    results = <SearchState kind="ERROR" />;
  } else if (courseDetail.kind === 'READY') {
    results = (
      <>
        <div className="bcsp-search-workspace__detail-actions">
          <ActionButton onClick={() => setCourseDetail({ kind: 'CLOSED' })} tone="quiet">
            &lt;&lt; {i18n.t('action.back')}
          </ActionButton>
          <p className="bcsp-search-workspace__route-meta">{i18n.t('search.course_detail_title')}</p>
        </div>
        <CourseDetailView
          onSectionNavigate={(key) => navigate(sectionHref(key))}
          response={courseDetail.response}
          sectionHref={sectionHref}
        />
      </>
    );
  } else if (empty) {
    results = <EmptySearchState />;
  } else if (query.kind === 'COURSES') {
    results = (
      <CourseResultsView
        onCourseDetail={openCourseDetail}
        onPageChange={(page) => void runSearch(page)}
        onSectionNavigate={(key) => navigate(sectionHref(key))}
        response={query.response}
        sectionHref={sectionHref}
      />
    );
  } else if (query.kind === 'SECTIONS') {
    results = (
      <SectionResultsView
        onCourseDetail={openCourseDetail}
        onPageChange={(page) => void runSearch(page)}
        onSectionNavigate={(key) => navigate(sectionHref(key))}
        response={query.response}
        sectionHref={sectionHref}
      />
    );
  } else {
    results = (
      <SearchState
        kind={query.kind}
        message={query.kind === 'VALIDATION_ERROR'
          ? i18n.t(filterSerializationIssueMessageKeys[query.issue])
          : undefined}
      />
    );
  }

  return (
    <div className="bcsp-search-workspace" data-search-mode={mode.toLowerCase()}>
      <SearchWorkspaceStyles />
      <section className="bcsp-search-workspace__filters" aria-labelledby="bcsp-search-filter-title">
        <header className="bcsp-search-workspace__header">
          <h3 id="bcsp-search-filter-title">{i18n.t('search.filters_title')}</h3>
          <p>{mode === 'COURSES' ? i18n.t('search.course_intro') : i18n.t('search.section_intro')}</p>
        </header>
        <FilterPanel
          disabled={query.kind === 'LOADING'}
          discovery={shellState.discovery}
          mode={mode}
          onChange={(next) => {
            setFilters(next);
            onFiltersChange?.(next);
            setQuery({ kind: 'IDLE' });
            setCourseDetail({ kind: 'CLOSED' });
          }}
          onSubmit={() => void runSearch(1)}
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
      <section className="bcsp-search-workspace__results" aria-labelledby="bcsp-search-results-title">
        <h3 className="bcsp-visually-hidden" id="bcsp-search-results-title">{i18n.t('search.results_title')}</h3>
        <div className="bcsp-search-workspace__state" data-query-state={query.kind.toLowerCase()}>
          {results}
        </div>
      </section>
    </div>
  );
}
