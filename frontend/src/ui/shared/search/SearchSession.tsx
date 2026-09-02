import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useReducer,
  useRef,
  type ReactNode,
} from 'react';

import type {
  CourseQueryRequestV1,
  CourseQueryResponseV1,
  FilterStateV1,
} from '../product';

export interface SearchScope {
  readonly term: string | null;
  readonly campuses: readonly string[];
}

interface SearchSessionState {
  readonly appliedScope: SearchScope | null;
  readonly candidateScope: SearchScope | null;
  readonly draftFilters: FilterStateV1 | null;
  readonly draftWasEdited: boolean;
  readonly expandedSectionDisclosures: ReadonlySet<string>;
  readonly lastSubmittedRequest: CourseQueryRequestV1 | null;
  readonly lastSuccessfulRequest: CourseQueryRequestV1 | null;
  readonly lastSuccessfulResponse: CourseQueryResponseV1 | null;
  /** True once the user changed or applied the scope; blocks a later stored-scope adoption. */
  readonly scopeTouched: boolean;
}

type SearchSessionAction =
  | {
    readonly type: 'INITIALIZE_SCOPE';
    readonly candidate: SearchScope;
    readonly applied: SearchScope | null;
    readonly filters: FilterStateV1;
  }
  | {
    readonly type: 'SET_CANDIDATE_SCOPE';
    readonly scope: SearchScope;
  }
  | {
    readonly type: 'APPLY_SCOPE';
    readonly scope: SearchScope;
    readonly filters: FilterStateV1;
  }
  | {
    readonly type: 'SET_DRAFT';
    readonly filters: FilterStateV1;
    readonly edited: boolean;
  }
  | {
    readonly type: 'SUBMIT';
    readonly request: CourseQueryRequestV1;
  }
  | {
    readonly type: 'SUCCEED';
    readonly request: CourseQueryRequestV1;
    readonly response: CourseQueryResponseV1;
  }
  | {
    readonly type: 'SET_SECTION_DISCLOSURE';
    readonly disclosureId: string;
    readonly expanded: boolean;
  };

const INITIAL_SEARCH_SESSION: SearchSessionState = {
  appliedScope: null,
  candidateScope: null,
  draftFilters: null,
  draftWasEdited: false,
  expandedSectionDisclosures: new Set<string>(),
  lastSubmittedRequest: null,
  lastSuccessfulRequest: null,
  lastSuccessfulResponse: null,
  scopeTouched: false,
};

function requestMatchesScope(
  request: CourseQueryRequestV1,
  scope: SearchScope | null,
): boolean {
  if (scope === null || request.filters.values.term !== scope.term) return false;
  const requestCampuses = [...request.filters.values.campuses].sort();
  const scopeCampuses = [...scope.campuses].sort();
  return requestCampuses.length === scopeCampuses.length
    && requestCampuses.every((campus, index) => campus === scopeCampuses[index]);
}

function reduceSearchSession(
  state: SearchSessionState,
  action: SearchSessionAction,
): SearchSessionState {
  if (action.type === 'INITIALIZE_SCOPE') {
    if (state.candidateScope !== null || state.draftFilters !== null) {
      if (
        state.appliedScope === null
        && !state.draftWasEdited
        && state.candidateScope?.term === null
        && action.candidate.term !== null
      ) {
        // Existing upgrade path: the term window arrived after the first
        // initialisation. It may now also carry a stored applied scope, but
        // never over a scope the user already touched.
        const applied = state.scopeTouched ? null : action.applied;
        return {
          ...state,
          appliedScope: applied,
          candidateScope: applied ?? action.candidate,
          draftFilters: action.filters,
        };
      }
      if (
        state.appliedScope === null
        && action.applied !== null
        && !state.draftWasEdited
        && !state.scopeTouched
        && state.lastSubmittedRequest === null
      ) {
        // Stored scope became ready later (its targets finished loading) while
        // the user has not touched the scope or the draft. Unlike APPLY_SCOPE
        // this never clears result state: nothing has been submitted yet.
        return {
          ...state,
          appliedScope: action.applied,
          candidateScope: action.applied,
          draftFilters: action.filters,
        };
      }
      return state;
    }
    return {
      ...state,
      appliedScope: action.applied,
      candidateScope: action.candidate,
      draftFilters: action.filters,
    };
  }
  if (action.type === 'SET_CANDIDATE_SCOPE') {
    return { ...state, candidateScope: action.scope, scopeTouched: true };
  }
  if (action.type === 'APPLY_SCOPE') {
    return {
      ...state,
      appliedScope: action.scope,
      candidateScope: action.scope,
      draftFilters: action.filters,
      draftWasEdited: true,
      expandedSectionDisclosures: new Set<string>(),
      lastSubmittedRequest: null,
      lastSuccessfulRequest: null,
      lastSuccessfulResponse: null,
      scopeTouched: true,
    };
  }
  if (action.type === 'SET_DRAFT') {
    return {
      ...state,
      draftFilters: action.filters,
      draftWasEdited: state.draftWasEdited || action.edited,
    };
  }
  if (action.type === 'SUBMIT') {
    return { ...state, lastSubmittedRequest: action.request };
  }
  if (action.type === 'SUCCEED') {
    if (
      state.lastSubmittedRequest !== action.request
      || !requestMatchesScope(action.request, state.appliedScope)
    ) {
      return state;
    }
    return {
      ...state,
      lastSuccessfulRequest: action.request,
      lastSuccessfulResponse: action.response,
    };
  }
  const expandedSectionDisclosures = new Set(state.expandedSectionDisclosures);
  if (action.expanded) expandedSectionDisclosures.add(action.disclosureId);
  else expandedSectionDisclosures.delete(action.disclosureId);
  return { ...state, expandedSectionDisclosures };
}

export interface SearchSessionRuntime {
  readonly state: SearchSessionState;
  readonly applyScope: (scope: SearchScope, filters: FilterStateV1) => void;
  readonly initializeScope: (
    candidate: SearchScope,
    applied: SearchScope | null,
    filters: FilterStateV1,
  ) => void;
  readonly recordSubmission: (request: CourseQueryRequestV1) => void;
  readonly recordSuccess: (
    request: CourseQueryRequestV1,
    response: CourseQueryResponseV1,
  ) => void;
  readonly restoreFilterScrollTop: () => number;
  readonly saveFilterScrollTop: (scrollTop: number) => void;
  readonly setDraftFilters: (filters: FilterStateV1, edited: boolean) => void;
  readonly setCandidateScope: (scope: SearchScope) => void;
  readonly setSectionDisclosureExpanded: (disclosureId: string, expanded: boolean) => void;
}

const SearchSessionContext = createContext<SearchSessionRuntime | null>(null);

export function SearchSessionProvider({ children }: { readonly children: ReactNode }) {
  const [state, dispatch] = useReducer(reduceSearchSession, INITIAL_SEARCH_SESSION);
  const filterScrollTop = useRef(0);
  const initializeScope = useCallback((
    candidate: SearchScope,
    applied: SearchScope | null,
    filters: FilterStateV1,
  ) => {
    dispatch({ type: 'INITIALIZE_SCOPE', applied, candidate, filters });
  }, []);
  const setCandidateScope = useCallback((scope: SearchScope) => {
    dispatch({ type: 'SET_CANDIDATE_SCOPE', scope });
  }, []);
  const applyScope = useCallback((scope: SearchScope, filters: FilterStateV1) => {
    dispatch({ type: 'APPLY_SCOPE', filters, scope });
  }, []);
  const setDraftFilters = useCallback((filters: FilterStateV1, edited: boolean) => {
    dispatch({ type: 'SET_DRAFT', edited, filters });
  }, []);
  const recordSubmission = useCallback((request: CourseQueryRequestV1) => {
    dispatch({ type: 'SUBMIT', request });
  }, []);
  const recordSuccess = useCallback((
    request: CourseQueryRequestV1,
    response: CourseQueryResponseV1,
  ) => {
    dispatch({ type: 'SUCCEED', request, response });
  }, []);
  const setSectionDisclosureExpanded = useCallback((
    disclosureId: string,
    expanded: boolean,
  ) => {
    dispatch({ type: 'SET_SECTION_DISCLOSURE', disclosureId, expanded });
  }, []);
  const saveFilterScrollTop = useCallback((scrollTop: number) => {
    filterScrollTop.current = Math.max(0, scrollTop);
  }, []);
  const restoreFilterScrollTop = useCallback(() => filterScrollTop.current, []);
  const value = useMemo<SearchSessionRuntime>(() => ({
    applyScope,
    initializeScope,
    recordSubmission,
    recordSuccess,
    restoreFilterScrollTop,
    saveFilterScrollTop,
    setDraftFilters,
    setCandidateScope,
    setSectionDisclosureExpanded,
    state,
  }), [
    applyScope,
    initializeScope,
    recordSubmission,
    recordSuccess,
    restoreFilterScrollTop,
    saveFilterScrollTop,
    setDraftFilters,
    setCandidateScope,
    setSectionDisclosureExpanded,
    state,
  ]);
  return <SearchSessionContext.Provider value={value}>{children}</SearchSessionContext.Provider>;
}

export function useSearchSession(): SearchSessionRuntime {
  const session = useContext(SearchSessionContext);
  if (session === null) throw new Error('SearchSessionProvider is missing');
  return session;
}

export function useOptionalSearchSession(): SearchSessionRuntime | null {
  return useContext(SearchSessionContext);
}
