import {
  ProductClient,
  type FilterRequestV2,
  type ProductClientOptions,
  type ProductClientPort,
  type SectionKey,
  type TraceId,
} from '../../shared/product';
import {
  parseLocalBootstrapData,
  type ConfirmUserDataResetRequest,
  type CreateSavedViewRequest,
  type CurrentFiltersCommand,
  type DuplicateSavedViewRequest,
  type EpisodeHistorySummary,
  type HistoryPage,
  type LocalBootstrapData,
  type LocalSettings,
  type LocalTermPullRequest,
  type LocalTermPullResponse,
  type PersonalResetResult,
  type PreparedUserDataReset,
  type RenameSavedViewRequest,
  type ReplaceCurrentFiltersRequest,
  type ReplaceSelectionRequest,
  type SavedViewCommand,
  type SavedViewDeleteResult,
  type SavedViewLibrary,
  type SavedViewMutation,
  type SavedViewsDeleteAllResult,
  type StoredCurrentFilters,
  type StoredSettings,
  type UpdateSavedViewRequest,
  type UpdateSettingsRequest,
} from './contracts';

export const LOCAL_PERSONAL_API_ROUTES = {
  bootstrap: { method: 'GET', path: '/api/v1/local/bootstrap' },
  settings: { method: 'GET', path: '/api/v1/local/settings' },
  selection: { method: 'GET', path: '/api/v1/local/selection' },
  history: { method: 'GET', path: '/api/v1/local/history' },
  currentFilters: { method: 'GET', path: '/api/v1/local/current-filters' },
  savedViews: { method: 'GET', path: '/api/v1/local/saved-views' },
  applySavedView: { method: 'POST', path: '/api/v1/local/saved-views/apply' },
  renameSavedView: { method: 'POST', path: '/api/v1/local/saved-views/rename' },
  updateSavedView: { method: 'POST', path: '/api/v1/local/saved-views/update' },
  duplicateSavedView: { method: 'POST', path: '/api/v1/local/saved-views/duplicate' },
  deleteSavedView: { method: 'POST', path: '/api/v1/local/saved-views/delete' },
  deleteAllSavedViews: { method: 'POST', path: '/api/v1/local/saved-views/delete-all' },
  resetCurrentFilters: { method: 'POST', path: '/api/v1/local/filters/reset' },
  prepareUserDataReset: { method: 'POST', path: '/api/v1/local/user-data-reset/prepare' },
  confirmUserDataReset: { method: 'POST', path: '/api/v1/local/user-data-reset/confirm' },
  pullTerm: { method: 'POST', path: '/api/v1/local/terms/pull' },
} as const;

export interface LocalPersonalApiPort {
  bootstrap(signal?: AbortSignal): Promise<LocalBootstrapData>;
  settings(signal?: AbortSignal): Promise<StoredSettings>;
  updateSettings(request: UpdateSettingsRequest, signal?: AbortSignal): Promise<StoredSettings>;
  selection(signal?: AbortSignal): Promise<readonly SectionKey[]>;
  replaceSelection(
    request: ReplaceSelectionRequest,
    signal?: AbortSignal,
  ): Promise<readonly SectionKey[]>;
  history(signal?: AbortSignal): Promise<HistoryPage<EpisodeHistorySummary>>;
  currentFilters(signal?: AbortSignal): Promise<StoredCurrentFilters>;
  replaceCurrentFilters(
    request: ReplaceCurrentFiltersRequest,
    signal?: AbortSignal,
  ): Promise<StoredCurrentFilters>;
  savedViews(signal?: AbortSignal): Promise<SavedViewLibrary>;
  createSavedView(
    request: CreateSavedViewRequest,
    signal?: AbortSignal,
  ): Promise<SavedViewMutation>;
  applySavedView(request: SavedViewCommand, signal?: AbortSignal): Promise<StoredCurrentFilters>;
  renameSavedView(
    request: RenameSavedViewRequest,
    signal?: AbortSignal,
  ): Promise<SavedViewMutation>;
  updateSavedView(
    request: UpdateSavedViewRequest,
    signal?: AbortSignal,
  ): Promise<SavedViewMutation>;
  duplicateSavedView(
    request: DuplicateSavedViewRequest,
    signal?: AbortSignal,
  ): Promise<SavedViewMutation>;
  deleteSavedView(
    request: SavedViewCommand,
    signal?: AbortSignal,
  ): Promise<SavedViewDeleteResult>;
  deleteAllSavedViews(
    request: CurrentFiltersCommand,
    signal?: AbortSignal,
  ): Promise<SavedViewsDeleteAllResult>;
  resetCurrentFilters(
    request: CurrentFiltersCommand,
    signal?: AbortSignal,
  ): Promise<StoredCurrentFilters>;
  prepareUserDataReset(
    expectedUserStateRevision: number,
    signal?: AbortSignal,
  ): Promise<PreparedUserDataReset>;
  confirmUserDataReset(
    confirmationToken: TraceId,
    signal?: AbortSignal,
  ): Promise<PersonalResetResult>;
  pullTerm(request: LocalTermPullRequest, signal?: AbortSignal): Promise<LocalTermPullResponse>;
}

export class LocalPersonalApi implements LocalPersonalApiPort {
  readonly #client: ProductClientPort;

  constructor(client: ProductClientPort) {
    this.#client = client;
  }

  async bootstrap(signal?: AbortSignal): Promise<LocalBootstrapData> {
    const data = await this.#client.get<unknown>(LOCAL_PERSONAL_API_ROUTES.bootstrap.path, signal);
    return parseLocalBootstrapData(data);
  }

  settings(signal?: AbortSignal): Promise<StoredSettings> {
    return this.#client.get<StoredSettings>(LOCAL_PERSONAL_API_ROUTES.settings.path, signal);
  }

  updateSettings(request: UpdateSettingsRequest, signal?: AbortSignal): Promise<StoredSettings> {
    return this.put(LOCAL_PERSONAL_API_ROUTES.settings.path, request, signal);
  }

  selection(signal?: AbortSignal): Promise<readonly SectionKey[]> {
    return this.#client.get<readonly SectionKey[]>(LOCAL_PERSONAL_API_ROUTES.selection.path, signal);
  }

  replaceSelection(
    request: ReplaceSelectionRequest,
    signal?: AbortSignal,
  ): Promise<readonly SectionKey[]> {
    return this.put(LOCAL_PERSONAL_API_ROUTES.selection.path, request, signal);
  }

  history(signal?: AbortSignal): Promise<HistoryPage<EpisodeHistorySummary>> {
    return this.#client.get<HistoryPage<EpisodeHistorySummary>>(
      LOCAL_PERSONAL_API_ROUTES.history.path,
      signal,
    );
  }

  currentFilters(signal?: AbortSignal): Promise<StoredCurrentFilters> {
    return this.#client.get<StoredCurrentFilters>(LOCAL_PERSONAL_API_ROUTES.currentFilters.path, signal);
  }

  replaceCurrentFilters(
    request: ReplaceCurrentFiltersRequest,
    signal?: AbortSignal,
  ): Promise<StoredCurrentFilters> {
    return this.put(LOCAL_PERSONAL_API_ROUTES.currentFilters.path, request, signal);
  }

  savedViews(signal?: AbortSignal): Promise<SavedViewLibrary> {
    return this.#client.get<SavedViewLibrary>(LOCAL_PERSONAL_API_ROUTES.savedViews.path, signal);
  }

  createSavedView(
    request: CreateSavedViewRequest,
    signal?: AbortSignal,
  ): Promise<SavedViewMutation> {
    return this.post(LOCAL_PERSONAL_API_ROUTES.savedViews.path, request, signal);
  }

  applySavedView(request: SavedViewCommand, signal?: AbortSignal): Promise<StoredCurrentFilters> {
    return this.post(LOCAL_PERSONAL_API_ROUTES.applySavedView.path, request, signal);
  }

  renameSavedView(
    request: RenameSavedViewRequest,
    signal?: AbortSignal,
  ): Promise<SavedViewMutation> {
    return this.post(LOCAL_PERSONAL_API_ROUTES.renameSavedView.path, request, signal);
  }

  updateSavedView(
    request: UpdateSavedViewRequest,
    signal?: AbortSignal,
  ): Promise<SavedViewMutation> {
    return this.post(LOCAL_PERSONAL_API_ROUTES.updateSavedView.path, request, signal);
  }

  duplicateSavedView(
    request: DuplicateSavedViewRequest,
    signal?: AbortSignal,
  ): Promise<SavedViewMutation> {
    return this.post(LOCAL_PERSONAL_API_ROUTES.duplicateSavedView.path, request, signal);
  }

  deleteSavedView(
    request: SavedViewCommand,
    signal?: AbortSignal,
  ): Promise<SavedViewDeleteResult> {
    return this.post(LOCAL_PERSONAL_API_ROUTES.deleteSavedView.path, request, signal);
  }

  deleteAllSavedViews(
    request: CurrentFiltersCommand,
    signal?: AbortSignal,
  ): Promise<SavedViewsDeleteAllResult> {
    return this.post(LOCAL_PERSONAL_API_ROUTES.deleteAllSavedViews.path, request, signal);
  }

  resetCurrentFilters(
    request: CurrentFiltersCommand,
    signal?: AbortSignal,
  ): Promise<StoredCurrentFilters> {
    return this.post(LOCAL_PERSONAL_API_ROUTES.resetCurrentFilters.path, request, signal);
  }

  prepareUserDataReset(
    expectedUserStateRevision: number,
    signal?: AbortSignal,
  ): Promise<PreparedUserDataReset> {
    return this.post(
      LOCAL_PERSONAL_API_ROUTES.prepareUserDataReset.path,
      { expectedUserStateRevision },
      signal,
    );
  }

  confirmUserDataReset(
    confirmationToken: TraceId,
    signal?: AbortSignal,
  ): Promise<PersonalResetResult> {
    return this.post(
      LOCAL_PERSONAL_API_ROUTES.confirmUserDataReset.path,
      { confirmationToken },
      signal,
    );
  }

  pullTerm(request: LocalTermPullRequest, signal?: AbortSignal): Promise<LocalTermPullResponse> {
    return this.post(LOCAL_PERSONAL_API_ROUTES.pullTerm.path, request, signal);
  }

  private put<TRequest, TResponse>(
    path: string,
    payload: TRequest,
    signal?: AbortSignal,
  ): Promise<TResponse> {
    return this.#client.request<TRequest, TResponse>(path, signal === undefined
      ? { method: 'PUT', payload }
      : { method: 'PUT', payload, signal });
  }

  private post<TRequest, TResponse>(
    path: string,
    payload: TRequest,
    signal?: AbortSignal,
  ): Promise<TResponse> {
    return this.#client.post<TRequest, TResponse>(path, payload, signal);
  }
}

export function createLocalPersonalApi(options: ProductClientOptions = {}): LocalPersonalApi {
  return new LocalPersonalApi(new ProductClient(options));
}

export type {
  ConfirmUserDataResetRequest,
  FilterRequestV2,
  LocalSettings,
};
