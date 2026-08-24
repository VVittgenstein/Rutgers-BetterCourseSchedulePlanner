use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bcsp_application::{
    PreparedQueryService, PreparedServingBinding, PreparedServingRegistry, PreparedServingSnapshot,
    ServiceStatusRegistry, SessionNonce, SharedQueryService, SharedWatchSocket,
    TargetRefreshDemand, is_product_campus, product_target_keys, product_term_publication,
};
use bcsp_contracts::{
    ContractDecodeError, FilterFieldId, FilterRequestV1, HttpRequestEnvelope, HttpSuccessEnvelope,
    InvalidDynamicFilterValueV3, SectionKey, ServiceTermPublicationV2, ServiceWorkStateV2,
    SystemTraceIdSource, TermCampusKey, TermId, TraceId, TraceIdSource,
    decode_versioned_envelope_json,
};
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
use bcsp_local_user_state::{
    CurrentFiltersRevision, HistoryFilter, LocalSettings, PageRequest, PersonalStateError,
    PersonalStateStore, SavedViewContent, SavedViewDefinition, SavedViewMatch, SavedViewReviewCode,
    SavedViewReviewReason, SavedViewRevision, SettingsRevision, UnixMillis, UserStateRevision,
};
use bcsp_operational_storage::OperationalStorage;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use time::OffsetDateTime;

use crate::{
    DesiredWatchCoordinator, DesiredWatchCoordinatorError, DesiredWatchMutationV1,
    LOCAL_DESIRED_WATCH_CONTRACT_VERSION, LocalApiErrorCode, LocalPrimaryDatabase,
    LocalSurfaceFailure, LocalSurfaceOutcome, LocalSurfaceState,
};

const LOCAL_USER_DATA_RESET_TOKEN_TTL: Duration = Duration::from_secs(60);

struct PendingLocalUserDataReset {
    token: TraceId,
    expected_state_revision: UserStateRevision,
    expires_at: Instant,
}

pub struct PersonalSurface {
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    mutation_store: Mutex<PersonalStateStore>,
    watch: Arc<SharedWatchSocket>,
    pending_reset: Mutex<Option<PendingLocalUserDataReset>>,
    target_refresh_demand: TargetRefreshDemand,
    service_status: Option<Arc<ServiceStatusRegistry>>,
    prepared_serving: Option<Arc<PreparedServingRegistry>>,
    desired_watch: Option<Arc<DesiredWatchCoordinator>>,
}

impl PersonalSurface {
    pub fn new(
        database: Arc<Mutex<LocalPrimaryDatabase>>,
        mutation_store: PersonalStateStore,
        watch: Arc<SharedWatchSocket>,
    ) -> Self {
        Self {
            database,
            mutation_store: Mutex::new(mutation_store),
            watch,
            pending_reset: Mutex::new(None),
            target_refresh_demand: TargetRefreshDemand::default(),
            service_status: None,
            prepared_serving: None,
            desired_watch: None,
        }
    }

    /// Attaches the desired-watch coordinator.
    ///
    /// Optional so isolated fixtures can build a surface without one; the
    /// production composition always supplies it, and without it the two
    /// desired-watch routes report the local build is not configured for
    /// them rather than inventing an empty authority.
    pub fn with_desired_watch(mut self, coordinator: Arc<DesiredWatchCoordinator>) -> Self {
        self.desired_watch = Some(coordinator);
        self
    }

    pub fn with_target_refresh_demand(
        mut self,
        target_refresh_demand: TargetRefreshDemand,
    ) -> Self {
        self.target_refresh_demand = target_refresh_demand;
        self
    }

    pub fn with_service_status(mut self, service_status: Arc<ServiceStatusRegistry>) -> Self {
        self.service_status = Some(service_status);
        self
    }

    pub fn with_prepared_serving(mut self, prepared: Arc<PreparedServingRegistry>) -> Self {
        self.prepared_serving = Some(prepared);
        self
    }

    fn coordinator(&self) -> Result<&DesiredWatchCoordinator, LocalSurfaceFailure> {
        self.desired_watch
            .as_deref()
            .ok_or_else(|| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))
    }

    fn prepared_snapshot(&self) -> Result<Option<PreparedServingBinding>, LocalSurfaceFailure> {
        match self.prepared_serving.as_ref() {
            // Legacy projection remains available only to isolated fixtures
            // that deliberately construct PersonalSurface without the
            // production prepared-serving registry.
            None => Ok(None),
            Some(registry) => registry.snapshot().map(Some).map_err(|_| {
                // A configured-but-rebuilding registry must never silently
                // fall back to a full Catalog projection while holding the
                // LocalPrimaryDatabase mutex.
                LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError)
            }),
        }
    }

    fn with_store<T>(
        &self,
        operation: impl FnOnce(&PersonalStateStore) -> Result<T, PersonalStateError>,
    ) -> Result<T, LocalSurfaceFailure> {
        let database = self.lock_database()?;
        operation(database.personal()).map_err(map_personal_error)
    }

    fn lock_database(&self) -> Result<MutexGuard<'_, LocalPrimaryDatabase>, LocalSurfaceFailure> {
        self.database
            .lock()
            .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))
    }

    fn with_mutation_store<T>(
        &self,
        operation: impl FnOnce(&mut PersonalStateStore) -> Result<T, PersonalStateError>,
    ) -> Result<T, LocalSurfaceFailure> {
        let mut store = self.lock_mutation_store()?;
        operation(&mut store).map_err(map_personal_error)
    }

    fn lock_mutation_store(
        &self,
    ) -> Result<MutexGuard<'_, PersonalStateStore>, LocalSurfaceFailure> {
        self.mutation_store
            .lock()
            .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))
    }
}

impl LocalSurfaceState for PersonalSurface {
    fn bootstrap(&self, nonce: &SessionNonce) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Bootstrap<'a, T> {
            mode: &'static str,
            session_nonce: &'a str,
            state: T,
        }

        let prepared = self.prepared_snapshot()?;
        let mut database = self.lock_database()?;
        let mut state = database
            .personal()
            .snapshot(PageRequest::DEFAULT)
            .map_err(map_personal_error)?;
        let current_raw = database
            .personal()
            .current_filters_raw_snapshot()
            .map_err(map_personal_error)?;
        let raw_snapshots = load_saved_view_raw_snapshots(database.personal(), &state.saved_views)
            .map_err(map_personal_error)?;
        if let Some(prepared) = prepared.as_deref() {
            drop(database);
            project_prepared_current_filter_compatibility(
                prepared,
                &mut state.current_filters,
                current_raw.as_ref(),
            );
            project_prepared_saved_view_dynamic_compatibility(
                prepared,
                &mut state.saved_views,
                &raw_snapshots,
            );
        } else {
            project_legacy_current_filter_compatibility(
                database.operational_mut(),
                &mut state.current_filters,
                current_raw.as_ref(),
            );
            project_legacy_saved_view_dynamic_compatibility(
                database.operational_mut(),
                &mut state.saved_views,
                &raw_snapshots,
            );
            drop(database);
        }
        state.active_watch_count = u8::try_from(self.watch.total_active_watch_count())
            .unwrap_or(bcsp_contracts::MAX_ACTIVE_WATCHES);
        encode(&Bootstrap {
            mode: "LOCAL",
            session_nonce: nonce.as_str(),
            state,
        })
    }

    /// The desired-watch authority read.
    ///
    /// One response, one snapshot, every row: the nine sections a user may
    /// desire plus the full removal history is 521 rows, and the whole point
    /// of a tombstone is that a page can tell "removed at revision N" from
    /// "never existed". Paginating would hand a page a partial view it could
    /// not distinguish from the complete one, so the response is bounded by
    /// proof instead -- see `LOCAL_DESIRED_WATCH_RESPONSE_BUDGET_BYTES`.
    fn desired_watch(&self) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let state = self
            .coordinator()?
            .read()
            .map_err(map_coordinator_error)?;
        encode(&state)
    }

    /// One desired-watch compare-and-swap, followed immediately by a
    /// reconcile, so the response describes what is actually running rather
    /// than what was asked for.
    fn put_desired_watch(
        &self,
        body: &[u8],
    ) -> Result<LocalSurfaceOutcome, LocalSurfaceFailure> {
        let mutation: DesiredWatchMutationV1 = decode_payload(body)?;
        if mutation.contract_version != LOCAL_DESIRED_WATCH_CONTRACT_VERSION {
            return Err(LocalSurfaceFailure::bad_request(
                LocalApiErrorCode::UnsupportedProtocolVersion,
            ));
        }
        let result = self
            .coordinator()?
            .submit(&mutation)
            .map_err(map_coordinator_error)?;
        Ok(LocalSurfaceOutcome {
            status: result.outcome.http_status(),
            body: encode(&result)?,
        })
    }

    fn settings(&self) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let settings = self.with_store(|store| store.settings())?;
        encode(&settings)
    }

    fn put_settings(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Update {
            expected_user_state_revision: UserStateRevision,
            expected_revision: u64,
            value: LocalSettings,
        }

        let update: Update = decode_payload(body)?;
        let revision =
            SettingsRevision::try_from(update.expected_revision).map_err(map_personal_error)?;
        let stored = self.with_mutation_store(|store| {
            store.compare_and_swap_settings(
                update.expected_user_state_revision,
                revision,
                &update.value,
            )
        })?;
        encode(&stored)
    }

    fn pull_term(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Request {
            contract_version: u16,
            term: TermId,
        }
        #[derive(Serialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        enum Disposition {
            Enqueued,
            AlreadyRequested,
            AlreadyReady,
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Response {
            contract_version: u16,
            term: TermId,
            disposition: Disposition,
            target_count: u64,
        }

        let request: Request = decode_payload(body)?;
        if request.contract_version != 1 {
            return Err(LocalSurfaceFailure::bad_request(
                LocalApiErrorCode::UnsupportedProtocolVersion,
            ));
        }
        let now = OffsetDateTime::now_utc();
        let window = RutgersTermWindow::at(now, RutgersTermWindowScope::Local)
            .map_err(|_| LocalSurfaceFailure::unprocessable(LocalApiErrorCode::TermOutOfRange))?;
        let visible = window
            .visible_terms()
            .iter()
            .find(|term| term.term() == &request.term)
            .filter(|term| !term.auto_managed())
            .ok_or_else(|| LocalSurfaceFailure::unprocessable(LocalApiErrorCode::TermOutOfRange))?;
        let mut database = self.lock_database()?;
        let published = database
            .operational_mut()
            .published_discovery_snapshot()
            .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))?;
        if product_term_publication(&published, visible.term(), now)
            != ServiceTermPublicationV2::Published
        {
            return Err(LocalSurfaceFailure::unprocessable(
                LocalApiErrorCode::TermNotPublished,
            ));
        }
        let targets = product_target_keys(visible.term());
        let missing_targets = targets
            .iter()
            .filter(|target| {
                !database
                    .operational_mut()
                    .complete_target_snapshot_state(target)
                    .is_ok_and(|state| state.ready)
            })
            .cloned()
            .collect::<Vec<_>>();
        drop(database);
        let active_targets = self
            .service_status
            .as_ref()
            .map(|status| {
                status
                    .snapshot()
                    .map(|snapshot| {
                        snapshot
                            .target_activities
                            .into_iter()
                            .filter(|activity| {
                                matches!(
                                    activity.work_state,
                                    ServiceWorkStateV2::Queued | ServiceWorkStateV2::Running
                                ) || (activity.work_state == ServiceWorkStateV2::RetryWait
                                    && activity.next_retry_at.is_some())
                            })
                            .map(|activity| activity.target)
                            .collect::<std::collections::BTreeSet<_>>()
                    })
                    .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))
            })
            .transpose()?
            .unwrap_or_default();
        let pullable_targets = missing_targets
            .iter()
            .filter(|target| !active_targets.contains(*target))
            .cloned()
            .collect::<Vec<_>>();
        let disposition = if missing_targets.is_empty() {
            Disposition::AlreadyReady
        } else {
            let term_enqueued = self
                .target_refresh_demand
                .request_manual_term(request.term.clone())
                .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))?;
            let target_retries = self
                .target_refresh_demand
                .request_manual_target_retries(&pullable_targets)
                .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))?;
            if term_enqueued || target_retries > 0 {
                Disposition::Enqueued
            } else {
                Disposition::AlreadyRequested
            }
        };
        encode(&Response {
            contract_version: 1,
            term: request.term,
            disposition,
            target_count: pullable_targets.len() as u64,
        })
    }

    fn selection(&self) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let selection = self.with_store(|store| store.selected_sections())?;
        encode(&selection)
    }

    fn put_selection(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Update {
            expected_user_state_revision: UserStateRevision,
            sections: Vec<SectionKey>,
        }

        let update: Update = decode_payload(body)?;
        let existing = self.with_store(|store| store.selected_sections())?;
        let existing = existing
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        let window =
            RutgersTermWindow::at(OffsetDateTime::now_utc(), RutgersTermWindowScope::Public)
                .map_err(|_| {
                    LocalSurfaceFailure::unprocessable(LocalApiErrorCode::TermOutOfRange)
                })?;
        if update.sections.iter().any(|section| {
            !existing.contains(section) && !is_product_campus(section.campus().as_str())
        }) {
            return Err(LocalSurfaceFailure::unprocessable(
                LocalApiErrorCode::InvalidLocalState,
            ));
        }
        if update.sections.iter().any(|section| {
            !existing.contains(section)
                && section.term() != window.current_term()
                && section.term() != window.next_term()
        }) {
            return Err(LocalSurfaceFailure::unprocessable(
                LocalApiErrorCode::TermOutOfRange,
            ));
        }
        self.with_mutation_store(|store| {
            store.replace_selected_sections(update.expected_user_state_revision, &update.sections)
        })?;
        encode(&update.sections)
    }

    fn history(&self) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let history = self.with_store(|store| {
            store.consistent_read(|store| {
                store.episode_history(&HistoryFilter::default(), PageRequest::DEFAULT)
            })
        })?;
        encode(&history)
    }

    fn current_filters(&self) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let prepared = self.prepared_snapshot()?;
        let mut database = self.lock_database()?;
        let (mut current, raw) = database
            .personal()
            .consistent_read(|store| {
                Ok((
                    store.current_filters()?,
                    store.current_filters_raw_snapshot()?,
                ))
            })
            .map_err(map_personal_error)?;
        if let Some(prepared) = prepared.as_deref() {
            drop(database);
            project_prepared_current_filter_compatibility(prepared, &mut current, raw.as_ref());
        } else {
            project_legacy_current_filter_compatibility(
                database.operational_mut(),
                &mut current,
                raw.as_ref(),
            );
        }
        encode(&current)
    }

    fn put_current_filters(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Update {
            expected_user_state_revision: UserStateRevision,
            expected_current_filters_revision: CurrentFiltersRevision,
            filters: FilterRequestV1,
        }

        let update: Update = decode_payload(body)?;
        let current = self.with_mutation_store(|store| {
            store.replace_current_filters(
                update.expected_user_state_revision,
                update.expected_current_filters_revision,
                &update.filters,
            )
        })?;
        encode(&current)
    }

    fn saved_views(&self) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SavedViewListItem {
            definition: SavedViewDefinition,
            match_state: SavedViewMatch,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SavedViewLibrary {
            state_revision: UserStateRevision,
            current_filters: bcsp_local_user_state::StoredCurrentFilters,
            views: Vec<SavedViewListItem>,
        }

        let prepared = self.prepared_snapshot()?;
        let mut database = self.lock_database()?;
        let (mut current_filters, current_raw, mut definitions, raw_snapshots) = database
            .personal()
            .consistent_read(|store| {
                let current_filters = store.current_filters()?;
                let current_raw = store.current_filters_raw_snapshot()?;
                let definitions = store.saved_views()?;
                let raw_snapshots = load_saved_view_raw_snapshots(store, &definitions)?;
                Ok((current_filters, current_raw, definitions, raw_snapshots))
            })
            .map_err(map_personal_error)?;
        if let Some(prepared) = prepared.as_deref() {
            drop(database);
            project_prepared_current_filter_compatibility(
                prepared,
                &mut current_filters,
                current_raw.as_ref(),
            );
            project_prepared_saved_view_dynamic_compatibility(
                prepared,
                &mut definitions,
                &raw_snapshots,
            );
        } else {
            project_legacy_current_filter_compatibility(
                database.operational_mut(),
                &mut current_filters,
                current_raw.as_ref(),
            );
            project_legacy_saved_view_dynamic_compatibility(
                database.operational_mut(),
                &mut definitions,
                &raw_snapshots,
            );
            drop(database);
        }
        let views = definitions
            .into_iter()
            .map(|definition| SavedViewListItem {
                match_state: definition.match_current(&current_filters),
                definition,
            })
            .collect();
        let library = SavedViewLibrary {
            state_revision: current_filters.state_revision,
            current_filters,
            views,
        };
        encode(&library)
    }

    fn create_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Create {
            expected_user_state_revision: UserStateRevision,
            expected_current_filters_revision: CurrentFiltersRevision,
            name: String,
            filters: FilterRequestV1,
        }

        let create: Create = decode_payload(body)?;
        let now = unix_millis_now()?;
        let mutation = self.with_mutation_store(|store| {
            store.create_saved_view(
                create.expected_user_state_revision,
                create.expected_current_filters_revision,
                &create.name,
                &create.filters,
                now,
            )
        })?;
        encode(&mutation)
    }

    fn apply_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let command: SavedViewCommand = decode_payload(body)?;
        let prepared = self.prepared_snapshot()?;
        let definition_and_raw_revision = {
            let database = self.lock_database()?;
            {
                let store = database.personal();
                let state_revision = store.user_state_revision().map_err(map_personal_error)?;
                let current_filters = store.current_filters().map_err(map_personal_error)?;
                if state_revision != command.expected_user_state_revision
                    || current_filters.revision != command.expected_current_filters_revision
                {
                    None
                } else {
                    let definition = store.saved_view(command.id).map_err(map_personal_error)?;
                    let raw_revision = store
                        .saved_view_raw_snapshot(command.id)
                        .map_err(map_personal_error)?
                        .map(|(revision, _)| revision);
                    definition.map(|definition| (definition, raw_revision))
                }
            }
        };
        let dynamically_incompatible =
            definition_and_raw_revision.is_some_and(|(definition, raw_revision)| {
                definition.revision == command.expected_view_revision
                    && if let Some(prepared) = prepared.as_deref() {
                        saved_view_apply_is_incompatible_prepared(
                            prepared,
                            &definition,
                            raw_revision,
                        )
                    } else {
                        match self.lock_database() {
                            Ok(mut database) => saved_view_apply_is_incompatible(
                                database.operational_mut(),
                                &definition,
                                raw_revision,
                            ),
                            Err(_) => true,
                        }
                    }
            });
        if dynamically_incompatible {
            return Err(LocalSurfaceFailure::unprocessable(
                LocalApiErrorCode::SavedViewIncompatible,
            ));
        }
        let current = self.with_mutation_store(|store| {
            store.apply_saved_view(
                command.expected_user_state_revision,
                command.expected_current_filters_revision,
                command.id,
                command.expected_view_revision,
            )
        })?;
        encode(&current)
    }

    fn rename_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Rename {
            expected_user_state_revision: UserStateRevision,
            expected_current_filters_revision: CurrentFiltersRevision,
            id: TraceId,
            expected_view_revision: SavedViewRevision,
            name: String,
        }

        let rename: Rename = decode_payload(body)?;
        let now = unix_millis_now()?;
        let mutation = self.with_mutation_store(|store| {
            store.rename_saved_view(
                rename.expected_user_state_revision,
                rename.expected_current_filters_revision,
                rename.id,
                rename.expected_view_revision,
                &rename.name,
                now,
            )
        })?;
        encode(&mutation)
    }

    fn update_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Update {
            expected_user_state_revision: UserStateRevision,
            expected_current_filters_revision: CurrentFiltersRevision,
            id: TraceId,
            expected_view_revision: SavedViewRevision,
            filters: FilterRequestV1,
        }

        let update: Update = decode_payload(body)?;
        let now = unix_millis_now()?;
        let mutation = self.with_mutation_store(|store| {
            store.update_saved_view(
                update.expected_user_state_revision,
                update.expected_current_filters_revision,
                update.id,
                update.expected_view_revision,
                &update.filters,
                now,
            )
        })?;
        encode(&mutation)
    }

    fn duplicate_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Duplicate {
            expected_user_state_revision: UserStateRevision,
            id: TraceId,
            expected_view_revision: SavedViewRevision,
            name: String,
        }

        let duplicate: Duplicate = decode_payload(body)?;
        let now = unix_millis_now()?;
        let mutation = self.with_mutation_store(|store| {
            store.duplicate_saved_view(
                duplicate.expected_user_state_revision,
                duplicate.id,
                duplicate.expected_view_revision,
                &duplicate.name,
                now,
            )
        })?;
        encode(&mutation)
    }

    fn delete_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let command: SavedViewCommand = decode_payload(body)?;
        let result = self.with_mutation_store(|store| {
            store.delete_saved_view(
                command.expected_user_state_revision,
                command.expected_current_filters_revision,
                command.id,
                command.expected_view_revision,
            )
        })?;
        encode(&result)
    }

    fn delete_all_saved_views(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let command: CurrentFiltersCommand = decode_payload(body)?;
        let result = self.with_mutation_store(|store| {
            store.delete_all_saved_views(
                command.expected_user_state_revision,
                command.expected_current_filters_revision,
            )
        })?;
        encode(&result)
    }

    fn reset_current_filters(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let command: CurrentFiltersCommand = decode_payload(body)?;
        let current = self.with_mutation_store(|store| {
            store.reset_current_filters(
                command.expected_user_state_revision,
                command.expected_current_filters_revision,
                None,
            )
        })?;
        encode(&current)
    }

    fn prepare_local_user_data_reset(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Prepare {
            expected_user_state_revision: UserStateRevision,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Prepared {
            confirmation_token: TraceId,
            expected_user_state_revision: UserStateRevision,
            expires_in_seconds: u64,
        }

        let prepare: Prepare = decode_payload(body)?;
        let mutation_store = self.lock_mutation_store()?;
        let current = mutation_store
            .user_state_revision()
            .map_err(map_personal_error)?;
        if current != prepare.expected_user_state_revision {
            return Err(LocalSurfaceFailure::revision_conflict(
                LocalApiErrorCode::UserStateRevisionConflict,
                current.get(),
            ));
        }
        let mut ids = SystemTraceIdSource;
        let token = ids.next_trace_id();
        let mut pending_reset = self
            .pending_reset
            .lock()
            .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))?;
        *pending_reset = Some(PendingLocalUserDataReset {
            token,
            expected_state_revision: current,
            expires_at: Instant::now() + LOCAL_USER_DATA_RESET_TOKEN_TTL,
        });
        drop(pending_reset);
        drop(mutation_store);
        encode(&Prepared {
            confirmation_token: token,
            expected_user_state_revision: current,
            expires_in_seconds: LOCAL_USER_DATA_RESET_TOKEN_TTL.as_secs(),
        })
    }

    fn confirm_local_user_data_reset(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Confirm {
            confirmation_token: TraceId,
        }

        let confirm: Confirm = decode_payload(body)?;
        let mut mutation_store = self.lock_mutation_store()?;
        let expected_state_revision = {
            let mut pending_reset = self
                .pending_reset
                .lock()
                .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))?;
            consume_reset_confirmation(
                &mut pending_reset,
                confirm.confirmation_token,
                Instant::now(),
            )?
        };

        // The reset is ONE lifecycle barrier over two owners, not two
        // independent clean-ups. Between stopping the watches and clearing
        // the rows, a page can attach and a reconcile can read rows that are
        // about to be deleted -- and arm them. The barrier is what makes that
        // impossible: nothing materializes from the moment it is raised, and
        // it is not lowered until every physical watch the process holds has
        // been stopped by identity and every record forgotten.
        let coordinator = self.desired_watch.as_deref();
        if let Some(coordinator) = coordinator {
            coordinator
                .begin_authority_reset()
                .map_err(map_coordinator_error)?;
        }
        self.watch.stop();
        self.watch.flush_dispatch_sink();
        let reset = mutation_store.clear_personal_data(expected_state_revision);
        // Lowered on BOTH paths. A reset that could not clear the database
        // must not leave the process holding watches whose records it has
        // forgotten, and must not leave materialization frozen for the rest
        // of the process's life either.
        if let Some(coordinator) = coordinator
            && let Err(error) = coordinator.finish_authority_reset()
        {
            tracing::error!(?error, "failed to lower the desired-watch reset barrier");
        }
        let reset = reset.map_err(map_personal_error)?;
        encode(&reset)
    }

    fn checkpoint_wal(&self) -> Result<(), LocalSurfaceFailure> {
        self.with_mutation_store(|store| store.checkpoint_wal())
            .map(|_| ())
    }
}

type SavedViewRawSnapshots = Vec<(TraceId, SavedViewRevision, JsonValue)>;

fn load_saved_view_raw_snapshots(
    store: &PersonalStateStore,
    definitions: &[SavedViewDefinition],
) -> Result<SavedViewRawSnapshots, PersonalStateError> {
    let mut snapshots = Vec::with_capacity(definitions.len());
    for definition in definitions {
        if definition.schema_version > 2 || definition.content.filters().is_none() {
            continue;
        }
        if let Some((revision, raw)) = store.saved_view_raw_snapshot(definition.id)? {
            snapshots.push((definition.id, revision, raw));
        }
    }
    Ok(snapshots)
}

fn project_prepared_saved_view_dynamic_compatibility(
    snapshot: &PreparedServingSnapshot,
    definitions: &mut [SavedViewDefinition],
    raw_snapshots: &SavedViewRawSnapshots,
) {
    project_legacy_saved_view_dynamic_compatibility_with(
        definitions,
        raw_snapshots,
        |definition| {
            prepared_review_reasons(
                snapshot,
                definition.content.filters(),
                OffsetDateTime::now_utc(),
            )
        },
    );
}

fn project_prepared_current_filter_compatibility(
    snapshot: &PreparedServingSnapshot,
    current: &mut bcsp_local_user_state::StoredCurrentFilters,
    raw: Option<&(CurrentFiltersRevision, u64, JsonValue)>,
) {
    project_legacy_current_filter_compatibility_with(current, raw, |filters| {
        prepared_review_reasons(snapshot, Some(filters), OffsetDateTime::now_utc())
    });
}

fn saved_view_apply_is_incompatible_prepared(
    snapshot: &PreparedServingSnapshot,
    definition: &SavedViewDefinition,
    raw_revision: Option<SavedViewRevision>,
) -> bool {
    saved_view_apply_is_incompatible_with(definition, raw_revision, |filters| {
        prepared_review_reasons(snapshot, Some(filters), OffsetDateTime::now_utc())
    })
}

fn prepared_review_reasons(
    snapshot: &PreparedServingSnapshot,
    filters: Option<&FilterRequestV1>,
    now: OffsetDateTime,
) -> Vec<SavedViewReviewReason> {
    let Some(filters) = filters else {
        return Vec::new();
    };
    if let Some(reason) = prepared_scope_unavailable_reason(snapshot, filters, now) {
        return vec![reason];
    }
    let values = filters.values();
    if values.subjects().is_empty()
        && values.keywords().is_none()
        && values.course_number_bands().is_empty()
        && values.levels().is_empty()
        && values.core().codes.is_empty()
        && values.instructors().is_empty()
        && values.meeting_locations().locations.is_empty()
        && values.exam_codes().is_empty()
    {
        return Vec::new();
    }
    let Some(targets) = prepared_saved_view_search_targets(snapshot, filters) else {
        return vec![scope_unavailable_reason(FilterFieldId::CourseCampus)];
    };
    PreparedQueryService::new(snapshot)
        .dynamic_filter_validation(&targets, values)
        .map(|response| dynamic_value_review_reasons(response.invalid_values))
        .unwrap_or_else(|_| vec![scope_unavailable_reason(FilterFieldId::CourseCampus)])
}

fn prepared_scope_unavailable_reason(
    snapshot: &PreparedServingSnapshot,
    filters: &FilterRequestV1,
    now: OffsetDateTime,
) -> Option<SavedViewReviewReason> {
    let Ok(window) = RutgersTermWindow::at(now, RutgersTermWindowScope::Local) else {
        return Some(scope_unavailable_reason(FilterFieldId::CourseTerm));
    };
    let values = filters.values();
    if !window
        .visible_terms()
        .iter()
        .any(|term| term.term() == values.term())
    {
        return Some(scope_unavailable_reason(FilterFieldId::CourseTerm));
    }
    let Some(targets) = prepared_saved_view_search_targets(snapshot, filters) else {
        return Some(scope_unavailable_reason(FilterFieldId::CourseCampus));
    };
    let applicable = targets
        .iter()
        .all(|target| snapshot.is_discovered(target) && snapshot.is_ready(target));
    (!applicable).then(|| scope_unavailable_reason(FilterFieldId::CourseCampus))
}

fn prepared_saved_view_search_targets(
    snapshot: &PreparedServingSnapshot,
    filters: &FilterRequestV1,
) -> Option<Vec<TermCampusKey>> {
    let values = filters.values();
    let targets = if values.campuses().is_empty() {
        snapshot
            .discovery()
            .targets
            .iter()
            .map(|target| target.key.clone())
            .filter(|target| target.term() == values.term())
            .filter(|target| is_product_campus(target.campus().as_str()))
            .collect::<Vec<_>>()
    } else {
        values
            .campuses()
            .iter()
            .cloned()
            .map(|campus| TermCampusKey::new(values.term().clone(), campus))
            .collect::<Vec<_>>()
    };
    (!targets.is_empty()).then_some(targets)
}

fn project_legacy_saved_view_dynamic_compatibility(
    storage: &mut OperationalStorage,
    definitions: &mut [SavedViewDefinition],
    raw_snapshots: &SavedViewRawSnapshots,
) {
    project_legacy_saved_view_dynamic_compatibility_with(
        definitions,
        raw_snapshots,
        |definition| legacy_review_reasons(storage, definition.content.filters()),
    );
}

fn project_legacy_saved_view_dynamic_compatibility_with(
    definitions: &mut [SavedViewDefinition],
    raw_snapshots: &SavedViewRawSnapshots,
    mut review_reasons: impl FnMut(&SavedViewDefinition) -> Vec<SavedViewReviewReason>,
) {
    for definition in definitions {
        if definition.content.filters().is_none() {
            continue;
        }
        let reasons = review_reasons(definition);
        let raw_snapshot = raw_snapshots
            .iter()
            .find(|(id, revision, _)| *id == definition.id && *revision == definition.revision)
            .map(|(_, _, raw_snapshot)| raw_snapshot);
        if raw_snapshot.is_none() && definition.schema_version <= 2 {
            definition.content = SavedViewContent::ReviewRequired {
                // Never attach another revision's raw JSON. `null` truthfully means that the
                // matching legacy snapshot was unavailable during this response.
                raw_snapshot: JsonValue::Null,
                reasons: vec![scope_unavailable_reason(FilterFieldId::CourseTerm)],
            };
            continue;
        }
        if !reasons.is_empty() {
            definition.content = SavedViewContent::ReviewRequired {
                // A current-schema row can also lack a matching raw sidecar
                // (for example a recovered or reset database). Preserve that
                // fact as JSON null instead of panicking or borrowing a
                // different revision's payload.
                raw_snapshot: raw_snapshot.cloned().unwrap_or(JsonValue::Null),
                reasons,
            };
        }
    }
}

fn project_legacy_current_filter_compatibility(
    storage: &mut OperationalStorage,
    current: &mut bcsp_local_user_state::StoredCurrentFilters,
    raw: Option<&(CurrentFiltersRevision, u64, JsonValue)>,
) {
    project_legacy_current_filter_compatibility_with(current, raw, |filters| {
        legacy_review_reasons(storage, Some(filters))
    });
}

fn project_legacy_current_filter_compatibility_with(
    current: &mut bcsp_local_user_state::StoredCurrentFilters,
    raw: Option<&(CurrentFiltersRevision, u64, JsonValue)>,
    review_reasons: impl FnOnce(&FilterRequestV1) -> Vec<SavedViewReviewReason>,
) {
    let Some(value) = current.value.as_mut() else {
        return;
    };
    if value.content.filters().is_none() {
        return;
    }
    let Some((raw_revision, schema_version, raw_snapshot)) = raw else {
        value.content = SavedViewContent::ReviewRequired {
            raw_snapshot: JsonValue::Null,
            reasons: vec![scope_unavailable_reason(FilterFieldId::CourseTerm)],
        };
        return;
    };
    if *schema_version > 2 {
        return;
    }
    if *raw_revision != current.revision {
        value.content = SavedViewContent::ReviewRequired {
            raw_snapshot: JsonValue::Null,
            reasons: vec![scope_unavailable_reason(FilterFieldId::CourseTerm)],
        };
        return;
    }
    let reasons = review_reasons(
        value
            .content
            .filters()
            .expect("compatible content was checked above"),
    );
    if reasons.is_empty() {
        return;
    }
    value.content = SavedViewContent::ReviewRequired {
        raw_snapshot: raw_snapshot.clone(),
        reasons,
    };
}

fn saved_view_apply_is_incompatible(
    storage: &mut OperationalStorage,
    definition: &SavedViewDefinition,
    raw_revision: Option<SavedViewRevision>,
) -> bool {
    saved_view_apply_is_incompatible_with(definition, raw_revision, |filters| {
        legacy_review_reasons(storage, Some(filters))
    })
}

fn saved_view_apply_is_incompatible_with(
    definition: &SavedViewDefinition,
    raw_revision: Option<SavedViewRevision>,
    review_reasons: impl FnOnce(&FilterRequestV1) -> Vec<SavedViewReviewReason>,
) -> bool {
    (definition.schema_version <= 2 && raw_revision != Some(definition.revision))
        || definition
            .content
            .filters()
            .is_none_or(|filters| !review_reasons(filters).is_empty())
}

fn legacy_review_reasons(
    storage: &mut OperationalStorage,
    filters: Option<&FilterRequestV1>,
) -> Vec<SavedViewReviewReason> {
    legacy_review_reasons_at(storage, filters, OffsetDateTime::now_utc())
}

fn legacy_review_reasons_at(
    storage: &mut OperationalStorage,
    filters: Option<&FilterRequestV1>,
    now: OffsetDateTime,
) -> Vec<SavedViewReviewReason> {
    let Some(filters) = filters else {
        return Vec::new();
    };
    if let Some(reason) = saved_view_scope_unavailable_reason_at(storage, filters, now) {
        return vec![reason];
    }
    saved_view_invalid_dynamic_reasons(storage, filters)
}

fn saved_view_invalid_dynamic_reasons(
    storage: &mut OperationalStorage,
    filters: &FilterRequestV1,
) -> Vec<SavedViewReviewReason> {
    saved_view_invalid_dynamic_reasons_with(storage, filters, |storage, targets, values| {
        SharedQueryService::new(storage)
            .dynamic_filter_validation(targets, values)
            .map(|response| response.invalid_values)
            .map_err(|_| ())
    })
}

fn saved_view_invalid_dynamic_reasons_with<E>(
    storage: &mut OperationalStorage,
    filters: &FilterRequestV1,
    validate: impl FnOnce(
        &mut OperationalStorage,
        &[TermCampusKey],
        &bcsp_contracts::NormalizedFilterValuesV1,
    ) -> Result<Vec<InvalidDynamicFilterValueV3>, E>,
) -> Vec<SavedViewReviewReason> {
    let values = filters.values();
    if values.subjects().is_empty()
        && values.keywords().is_none()
        && values.course_number_bands().is_empty()
        && values.levels().is_empty()
        && values.core().codes.is_empty()
        && values.instructors().is_empty()
        && values.meeting_locations().locations.is_empty()
        && values.exam_codes().is_empty()
    {
        return Vec::new();
    }
    let Some(targets) = saved_view_search_targets(storage, filters) else {
        return vec![scope_unavailable_reason(FilterFieldId::CourseCampus)];
    };
    match validate(storage, &targets, values) {
        Ok(invalid_values) => dynamic_value_review_reasons(invalid_values),
        Err(_) => vec![scope_unavailable_reason(FilterFieldId::CourseCampus)],
    }
}

fn dynamic_value_review_reasons(
    invalid_values: impl IntoIterator<Item = InvalidDynamicFilterValueV3>,
) -> Vec<SavedViewReviewReason> {
    invalid_values
        .into_iter()
        .map(|invalid| invalid.field)
        .collect::<std::collections::BTreeSet<FilterFieldId>>()
        .into_iter()
        .map(|field| SavedViewReviewReason {
            stable_id: field.wire_name().to_owned(),
            code: SavedViewReviewCode::DynamicValueUnavailable,
        })
        .collect()
}

fn saved_view_scope_unavailable_reason_at(
    storage: &mut OperationalStorage,
    filters: &FilterRequestV1,
    now: OffsetDateTime,
) -> Option<SavedViewReviewReason> {
    let Ok(window) = RutgersTermWindow::at(now, RutgersTermWindowScope::Local) else {
        return Some(scope_unavailable_reason(FilterFieldId::CourseTerm));
    };
    let values = filters.values();
    if !window
        .visible_terms()
        .iter()
        .any(|term| term.term() == values.term())
    {
        return Some(scope_unavailable_reason(FilterFieldId::CourseTerm));
    }
    let Ok(discovered) = storage.discovered_targets() else {
        return Some(scope_unavailable_reason(FilterFieldId::CourseCampus));
    };
    let published = discovered
        .into_iter()
        .filter(|target| target.term() == values.term())
        .filter(|target| is_product_campus(target.campus().as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    let targets = if values.campuses().is_empty() {
        published.iter().cloned().collect::<Vec<_>>()
    } else {
        values
            .campuses()
            .iter()
            .cloned()
            .map(|campus| TermCampusKey::new(values.term().clone(), campus))
            .collect::<Vec<_>>()
    };
    let applicable = !targets.is_empty()
        && targets.iter().all(|target| published.contains(target))
        && targets.iter().all(|target| {
            storage
                .complete_target_snapshot_state(target)
                .is_ok_and(|state| state.ready)
        });
    (!applicable).then(|| scope_unavailable_reason(FilterFieldId::CourseCampus))
}

fn scope_unavailable_reason(field: FilterFieldId) -> SavedViewReviewReason {
    SavedViewReviewReason {
        stable_id: field.wire_name().to_owned(),
        code: SavedViewReviewCode::ScopeUnavailable,
    }
}

fn saved_view_search_targets(
    storage: &OperationalStorage,
    filters: &FilterRequestV1,
) -> Option<Vec<TermCampusKey>> {
    let values = filters.values();
    let targets: Vec<TermCampusKey> = if values.campuses().is_empty() {
        storage
            .discovered_targets()
            .ok()?
            .into_iter()
            .filter(|target| target.term() == values.term())
            .filter(|target| is_product_campus(target.campus().as_str()))
            .collect()
    } else {
        values
            .campuses()
            .iter()
            .cloned()
            .map(|campus| TermCampusKey::new(values.term().clone(), campus))
            .collect()
    };
    (!targets.is_empty()).then_some(targets)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CurrentFiltersCommand {
    expected_user_state_revision: UserStateRevision,
    expected_current_filters_revision: CurrentFiltersRevision,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedViewCommand {
    expected_user_state_revision: UserStateRevision,
    expected_current_filters_revision: CurrentFiltersRevision,
    id: TraceId,
    expected_view_revision: SavedViewRevision,
}

fn unix_millis_now() -> Result<UnixMillis, LocalSurfaceFailure> {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))?
        .as_millis();
    let milliseconds = i64::try_from(milliseconds)
        .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))?;
    UnixMillis::try_from(milliseconds)
        .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))
}

fn consume_reset_confirmation(
    pending: &mut Option<PendingLocalUserDataReset>,
    provided_token: TraceId,
    now: Instant,
) -> Result<UserStateRevision, LocalSurfaceFailure> {
    let Some(candidate) = pending.take() else {
        return Err(LocalSurfaceFailure::conflict(
            LocalApiErrorCode::ResetConfirmationRequired,
        ));
    };
    if now >= candidate.expires_at {
        return Err(LocalSurfaceFailure::conflict(
            LocalApiErrorCode::ResetConfirmationExpired,
        ));
    }
    if provided_token != candidate.token {
        *pending = Some(candidate);
        return Err(LocalSurfaceFailure::conflict(
            LocalApiErrorCode::ResetConfirmationInvalid,
        ));
    }
    Ok(candidate.expected_state_revision)
}

fn decode_payload<T>(body: &[u8]) -> Result<T, LocalSurfaceFailure>
where
    T: for<'de> Deserialize<'de>,
{
    decode_versioned_envelope_json::<HttpRequestEnvelope<T>>(body)
        .map(HttpRequestEnvelope::into_payload)
        .map_err(|error| match error {
            ContractDecodeError::UnsupportedProtocolVersion { .. } => {
                LocalSurfaceFailure::bad_request(LocalApiErrorCode::UnsupportedProtocolVersion)
            }
            ContractDecodeError::MalformedRequest => {
                LocalSurfaceFailure::bad_request(LocalApiErrorCode::MalformedRequest)
            }
        })
}

fn encode<T>(value: &T) -> Result<Vec<u8>, LocalSurfaceFailure>
where
    T: Serialize,
{
    serde_json::to_vec(&HttpSuccessEnvelope::new(value))
        .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))
}

/// Maps a coordinator failure onto a status.
///
/// A busy database is 503 and not 500: nothing is broken, another writer
/// simply holds it, and the same request succeeds on retry. Reporting it as
/// an internal error would tell a page to give up on a command that is still
/// perfectly submittable -- and, worse, would look identical to a real fault.
fn map_coordinator_error(error: DesiredWatchCoordinatorError) -> LocalSurfaceFailure {
    match error {
        DesiredWatchCoordinatorError::Storage(error) => map_personal_error(error),
        DesiredWatchCoordinatorError::Poisoned => {
            LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError)
        }
    }
}

fn map_personal_error(error: PersonalStateError) -> LocalSurfaceFailure {
    match error {
        PersonalStateError::RevisionConflict { actual, .. } => {
            LocalSurfaceFailure::revision_conflict(
                LocalApiErrorCode::SettingsRevisionConflict,
                actual,
            )
        }
        PersonalStateError::UserStateRevisionConflict { actual, .. } => {
            LocalSurfaceFailure::revision_conflict(
                LocalApiErrorCode::UserStateRevisionConflict,
                actual,
            )
        }
        PersonalStateError::CurrentFiltersRevisionConflict { actual, .. } => {
            LocalSurfaceFailure::revision_conflict(
                LocalApiErrorCode::CurrentFiltersRevisionConflict,
                actual,
            )
        }
        PersonalStateError::SavedViewRevisionConflict { actual, .. } => {
            LocalSurfaceFailure::revision_conflict(
                LocalApiErrorCode::SavedViewRevisionConflict,
                actual,
            )
        }
        PersonalStateError::SavedViewNameConflict {
            existing_revision, ..
        } => LocalSurfaceFailure::revision_conflict(
            LocalApiErrorCode::SavedViewNameConflict,
            existing_revision,
        ),
        PersonalStateError::SavedViewNotFound { .. } => {
            LocalSurfaceFailure::not_found(LocalApiErrorCode::SavedViewNotFound)
        }
        PersonalStateError::SavedViewIncompatible { .. } => {
            LocalSurfaceFailure::unprocessable(LocalApiErrorCode::SavedViewIncompatible)
        }
        PersonalStateError::InvalidSavedViewName | PersonalStateError::InvalidFilterSnapshot => {
            LocalSurfaceFailure::bad_request(LocalApiErrorCode::InvalidSavedView)
        }
        PersonalStateError::StorageFull => {
            LocalSurfaceFailure::insufficient_storage(LocalApiErrorCode::StorageFull)
        }
        PersonalStateError::InvalidSetting(_)
        | PersonalStateError::DuplicateSelection(_)
        | PersonalStateError::SelectionLimitExceeded { .. }
        | PersonalStateError::InvalidPageLimit
        | PersonalStateError::InvalidPageOffset
        | PersonalStateError::InvalidEpisodeSummary => {
            LocalSurfaceFailure::bad_request(LocalApiErrorCode::InvalidLocalState)
        }
        error if error.is_storage_busy() => {
            LocalSurfaceFailure::service_unavailable(LocalApiErrorCode::StorageBusy)
        }
        _ => LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use bcsp_contracts::{
        CampusCode, FilterTokenV1, FilterValuesInputV1, NormalizedFilterValuesV1, TermId,
    };
    use serde_json::json;

    use super::*;

    fn trace(value: u64) -> TraceId {
        TraceId::from_str(&format!("00000000-0000-4000-8000-{value:012x}")).unwrap()
    }

    #[test]
    fn expired_reset_confirmation_is_consumed_and_rejected() {
        let token = trace(1);
        let mut pending = Some(PendingLocalUserDataReset {
            token,
            expected_state_revision: UserStateRevision::try_from(7).unwrap(),
            expires_at: Instant::now() - Duration::from_millis(1),
        });
        assert_eq!(
            consume_reset_confirmation(&mut pending, token, Instant::now()),
            Err(LocalSurfaceFailure::conflict(
                LocalApiErrorCode::ResetConfirmationExpired
            ))
        );
        assert!(pending.is_none());
    }

    #[test]
    fn wrong_reset_token_does_not_consume_the_real_single_use_token() {
        let token = trace(2);
        let expected = UserStateRevision::try_from(8).unwrap();
        let now = Instant::now();
        let mut pending = Some(PendingLocalUserDataReset {
            token,
            expected_state_revision: expected,
            expires_at: now + Duration::from_secs(1),
        });
        assert_eq!(
            consume_reset_confirmation(&mut pending, trace(3), now),
            Err(LocalSurfaceFailure::conflict(
                LocalApiErrorCode::ResetConfirmationInvalid
            ))
        );
        assert_eq!(
            consume_reset_confirmation(&mut pending, token, now),
            Ok(expected)
        );
        assert!(pending.is_none());
    }

    #[test]
    fn invalid_legacy_dynamic_options_project_every_reason_with_the_original_snapshot() {
        let id = trace(4);
        let revision = SavedViewRevision::try_from(3).unwrap();
        let mut input =
            FilterValuesInputV1::for_term(TermId::try_from("92026").expect("synthetic term"));
        input.campuses = vec![CampusCode::try_from("NB").expect("synthetic campus")];
        input.instructors =
            vec![FilterTokenV1::try_from("Removed Instructor").expect("valid token")];
        let filters =
            FilterRequestV1::new(NormalizedFilterValuesV1::try_new(input).expect("valid filters"));
        let definition = SavedViewDefinition {
            id,
            name: "Legacy".to_owned(),
            schema_version: 1,
            revision,
            content: SavedViewContent::Compatible {
                filters: Box::new(filters),
            },
            created_at: UnixMillis::try_from(1).unwrap(),
            updated_at: UnixMillis::try_from(2).unwrap(),
        };
        let raw = json!({
            "codecVersion": 1,
            "schemaVersion": 1,
            "fields": {"FLT-S06": ["Removed Instructor"]},
        });
        let mut definitions = vec![definition.clone()];
        project_legacy_saved_view_dynamic_compatibility_with(
            &mut definitions,
            &vec![(id, revision, raw.clone())],
            |_| {
                vec![
                    SavedViewReviewReason {
                        stable_id: "FLT-C03".to_owned(),
                        code: SavedViewReviewCode::DynamicValueUnavailable,
                    },
                    SavedViewReviewReason {
                        stable_id: "FLT-S05".to_owned(),
                        code: SavedViewReviewCode::DynamicValueUnavailable,
                    },
                ]
            },
        );
        assert_eq!(
            definitions[0].content,
            SavedViewContent::ReviewRequired {
                raw_snapshot: raw,
                reasons: vec![
                    SavedViewReviewReason {
                        stable_id: "FLT-C03".to_owned(),
                        code: SavedViewReviewCode::DynamicValueUnavailable,
                    },
                    SavedViewReviewReason {
                        stable_id: "FLT-S05".to_owned(),
                        code: SavedViewReviewCode::DynamicValueUnavailable,
                    },
                ],
            }
        );

        let mut raced = vec![definition.clone()];
        project_legacy_saved_view_dynamic_compatibility_with(
            &mut raced,
            &vec![(
                id,
                SavedViewRevision::try_from(4).unwrap(),
                json!({"wrongRevision": true}),
            )],
            |_| {
                vec![SavedViewReviewReason {
                    stable_id: "FLT-S05".to_owned(),
                    code: SavedViewReviewCode::DynamicValueUnavailable,
                }]
            },
        );
        assert_eq!(
            raced[0].content,
            SavedViewContent::ReviewRequired {
                raw_snapshot: JsonValue::Null,
                reasons: vec![scope_unavailable_reason(FilterFieldId::CourseTerm)],
            },
            "a raw snapshot from another revision is never attached and the view fails closed"
        );

        let mut missing = vec![definition];
        project_legacy_saved_view_dynamic_compatibility_with(&mut missing, &Vec::new(), |_| {
            Vec::new()
        });
        assert_eq!(
            missing[0].content,
            SavedViewContent::ReviewRequired {
                raw_snapshot: JsonValue::Null,
                reasons: vec![scope_unavailable_reason(FilterFieldId::CourseTerm)],
            },
            "a missing legacy raw snapshot cannot leave a temporarily compatible view",
        );
    }

    #[test]
    fn exact_validation_invalid_values_become_stably_sorted_unique_field_reasons() {
        let reasons = dynamic_value_review_reasons([
            InvalidDynamicFilterValueV3 {
                field: FilterFieldId::SectionInstructor,
                value: "Removed A".to_owned(),
            },
            InvalidDynamicFilterValueV3 {
                field: FilterFieldId::CourseSubject,
                value: "999".to_owned(),
            },
            InvalidDynamicFilterValueV3 {
                field: FilterFieldId::SectionInstructor,
                value: "Removed B".to_owned(),
            },
            InvalidDynamicFilterValueV3 {
                field: FilterFieldId::SectionExam,
                value: "X".to_owned(),
            },
        ]);
        assert_eq!(
            reasons,
            vec![
                SavedViewReviewReason {
                    stable_id: "FLT-C03".to_owned(),
                    code: SavedViewReviewCode::DynamicValueUnavailable,
                },
                SavedViewReviewReason {
                    stable_id: "FLT-S05".to_owned(),
                    code: SavedViewReviewCode::DynamicValueUnavailable,
                },
                SavedViewReviewReason {
                    stable_id: "FLT-S09".to_owned(),
                    code: SavedViewReviewCode::DynamicValueUnavailable,
                },
            ]
        );
    }

    #[test]
    fn legacy_scope_reasons_distinguish_term_from_campus_readiness() {
        const RC4_FIXED_NOW_UNIX: i64 = 1_784_289_600;
        let now = OffsetDateTime::from_unix_timestamp(RC4_FIXED_NOW_UNIX)
            .expect("2026-07-17T12:00:00Z fixed clock");
        let mut storage = OperationalStorage::open_in_memory().expect("storage");
        let out_of_window = FilterRequestV1::new(
            NormalizedFilterValuesV1::try_new(FilterValuesInputV1::for_term(
                TermId::try_from("02025").expect("old term"),
            ))
            .expect("filters"),
        );
        assert_eq!(
            legacy_review_reasons_at(&mut storage, Some(&out_of_window), now),
            vec![scope_unavailable_reason(FilterFieldId::CourseTerm)],
        );

        let window = RutgersTermWindow::at(now, RutgersTermWindowScope::Local)
            .expect("2026-07-17 bundled current window");
        assert_eq!(window.current_term().as_str(), "72026");
        let mut input = FilterValuesInputV1::for_term(window.current_term().clone());
        input.campuses = vec![CampusCode::try_from("NB").expect("NB")];
        let unavailable_target =
            FilterRequestV1::new(NormalizedFilterValuesV1::try_new(input).expect("filters"));
        assert_eq!(
            legacy_review_reasons_at(&mut storage, Some(&unavailable_target), now),
            vec![scope_unavailable_reason(FilterFieldId::CourseCampus)],
            "a selected target that is unpublished or not READY belongs to FLT-C02",
        );
    }

    #[test]
    fn exact_validation_error_or_race_fails_closed_for_current_saved_and_apply_paths() {
        let revision = SavedViewRevision::try_from(7).expect("view revision");
        let current_revision = CurrentFiltersRevision::try_from(5).expect("current revision");
        let mut input = FilterValuesInputV1::for_term(TermId::try_from("92026").expect("term"));
        input.campuses = vec![CampusCode::try_from("NB").expect("NB")];
        input.instructors = vec![FilterTokenV1::try_from("Grace Hopper").expect("instructor")];
        let filters =
            FilterRequestV1::new(NormalizedFilterValuesV1::try_new(input).expect("filters"));
        let mut storage = OperationalStorage::open_in_memory().expect("storage");
        let reasons = saved_view_invalid_dynamic_reasons_with(&mut storage, &filters, |_, _, _| {
            Err::<Vec<InvalidDynamicFilterValueV3>, _>("CONTENT_VERSION_RACE")
        });
        assert_eq!(
            reasons,
            vec![scope_unavailable_reason(FilterFieldId::CourseCampus)],
            "an unverified dynamic value is never labeled valid",
        );
        let raw = json!({"legacy": "original-and-preserved"});
        let definition = SavedViewDefinition {
            id: trace(8),
            name: "Race".to_owned(),
            schema_version: 2,
            revision,
            content: SavedViewContent::Compatible {
                filters: Box::new(filters.clone()),
            },
            created_at: UnixMillis::try_from(1).unwrap(),
            updated_at: UnixMillis::try_from(2).unwrap(),
        };
        let mut saved = vec![definition.clone()];
        project_legacy_saved_view_dynamic_compatibility_with(
            &mut saved,
            &vec![(definition.id, revision, raw.clone())],
            |_| reasons.clone(),
        );
        assert_eq!(
            saved[0].content,
            SavedViewContent::ReviewRequired {
                raw_snapshot: raw.clone(),
                reasons: reasons.clone(),
            },
            "the exact same-revision legacy JSON remains available for review",
        );

        let mut current = bcsp_local_user_state::StoredCurrentFilters {
            state_revision: UserStateRevision::try_from(1).expect("state revision"),
            revision: current_revision,
            value: Some(bcsp_local_user_state::CurrentFilters {
                association: bcsp_local_user_state::FilterAssociation::Custom,
                content: SavedViewContent::Compatible {
                    filters: Box::new(filters),
                },
            }),
        };
        let current_raw = (current_revision, 2, raw.clone());
        project_legacy_current_filter_compatibility_with(&mut current, Some(&current_raw), |_| {
            reasons.clone()
        });
        assert_eq!(
            current.value.expect("current value").content,
            SavedViewContent::ReviewRequired {
                raw_snapshot: raw,
                reasons: reasons.clone(),
            },
        );
        assert!(
            saved_view_apply_is_incompatible_with(&definition, Some(revision), |_| reasons),
            "Apply independently rejects an exact validation error/race",
        );
    }

    #[test]
    fn missing_or_mismatched_legacy_raw_is_fail_closed_without_attaching_wrong_json() {
        let revision = SavedViewRevision::try_from(3).expect("view revision");
        let current_revision = CurrentFiltersRevision::try_from(4).expect("current revision");
        let filters = FilterRequestV1::new(
            NormalizedFilterValuesV1::try_new(FilterValuesInputV1::for_term(
                TermId::try_from("92026").expect("term"),
            ))
            .expect("filters"),
        );
        let definition = SavedViewDefinition {
            id: trace(9),
            name: "Missing raw".to_owned(),
            schema_version: 2,
            revision,
            content: SavedViewContent::Compatible {
                filters: Box::new(filters.clone()),
            },
            created_at: UnixMillis::try_from(1).unwrap(),
            updated_at: UnixMillis::try_from(2).unwrap(),
        };
        assert!(saved_view_apply_is_incompatible_with(
            &definition,
            None,
            |_| Vec::new(),
        ));
        assert!(saved_view_apply_is_incompatible_with(
            &definition,
            Some(SavedViewRevision::try_from(2).expect("other revision")),
            |_| Vec::new(),
        ));
        assert!(
            !saved_view_apply_is_incompatible_with(&definition, Some(revision), |_| Vec::new()),
            "only the exact raw revision plus successful validation may pass",
        );

        let mut current = bcsp_local_user_state::StoredCurrentFilters {
            state_revision: UserStateRevision::try_from(1).expect("state revision"),
            revision: current_revision,
            value: Some(bcsp_local_user_state::CurrentFilters {
                association: bcsp_local_user_state::FilterAssociation::Custom,
                content: SavedViewContent::Compatible {
                    filters: Box::new(filters),
                },
            }),
        };
        project_legacy_current_filter_compatibility_with(&mut current, None, |_| Vec::new());
        assert_eq!(
            current.value.expect("current value").content,
            SavedViewContent::ReviewRequired {
                raw_snapshot: JsonValue::Null,
                reasons: vec![scope_unavailable_reason(FilterFieldId::CourseTerm)],
            },
            "missing current-filter raw cannot remain auto-applicable",
        );
    }
}
