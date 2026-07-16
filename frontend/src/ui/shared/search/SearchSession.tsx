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

interface SearchSessionState {
  readonly draftFilters: FilterStateV1 | null;
  readonly draftWasEdited: boolean;
  readonly expandedSectionDisclosures: ReadonlySet<string>;
  readonly lastSubmittedRequest: CourseQueryRequestV1 | null;
  readonly lastSuccessfulRequest: CourseQueryRequestV1 | null;
  readonly lastSuccessfulResponse: CourseQueryResponseV1 | null;
}

type SearchSessionAction =
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
  draftFilters: null,
  draftWasEdited: false,
  expandedSectionDisclosures: new Set<string>(),
  lastSubmittedRequest: null,
  lastSuccessfulRequest: null,
  lastSuccessfulResponse: null,
};

function reduceSearchSession(
  state: SearchSessionState,
  action: SearchSessionAction,
): SearchSessionState {
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

interface SearchSessionRuntime {
  readonly state: SearchSessionState;
  readonly recordSubmission: (request: CourseQueryRequestV1) => void;
  readonly recordSuccess: (
    request: CourseQueryRequestV1,
    response: CourseQueryResponseV1,
  ) => void;
  readonly restoreFilterScrollTop: () => number;
  readonly saveFilterScrollTop: (scrollTop: number) => void;
  readonly setDraftFilters: (filters: FilterStateV1, edited: boolean) => void;
  readonly setSectionDisclosureExpanded: (disclosureId: string, expanded: boolean) => void;
}

const SearchSessionContext = createContext<SearchSessionRuntime | null>(null);

export function SearchSessionProvider({ children }: { readonly children: ReactNode }) {
  const [state, dispatch] = useReducer(reduceSearchSession, INITIAL_SEARCH_SESSION);
  const filterScrollTop = useRef(0);
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
  const restoreFilterScrollTop = useCallback(() => filterScrollTop.current, []);
  const saveFilterScrollTop = useCallback((scrollTop: number) => {
    filterScrollTop.current = scrollTop;
  }, []);
  const value = useMemo<SearchSessionRuntime>(() => ({
    recordSubmission,
    recordSuccess,
    restoreFilterScrollTop,
    saveFilterScrollTop,
    setDraftFilters,
    setSectionDisclosureExpanded,
    state,
  }), [
    recordSubmission,
    recordSuccess,
    restoreFilterScrollTop,
    saveFilterScrollTop,
    setDraftFilters,
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
