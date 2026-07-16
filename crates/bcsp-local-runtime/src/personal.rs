use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bcsp_application::{SessionNonce, SharedQueryError, SharedQueryService, SharedWatchSocket};
use bcsp_contracts::{
    ContractDecodeError, FilterRequestV1, HttpRequestEnvelope, HttpSuccessEnvelope, SectionKey,
    SystemTraceIdSource, TermCampusKey, TraceId, TraceIdSource, decode_versioned_envelope_json,
};
use bcsp_local_user_state::{
    CurrentFiltersRevision, HistoryFilter, LocalSettings, PageRequest, PersonalStateError,
    PersonalStateStore, SavedViewContent, SavedViewDefinition, SavedViewIncompatibility,
    SavedViewMatch, SavedViewRevision, SettingsRevision, UnixMillis, UserStateRevision,
};
use bcsp_operational_storage::OperationalStorage;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{LocalApiErrorCode, LocalPrimaryDatabase, LocalSurfaceFailure, LocalSurfaceState};

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

        let mut database = self.lock_database()?;
        let mut state = database
            .personal()
            .snapshot(PageRequest::DEFAULT)
            .map_err(map_personal_error)?;
        let raw_snapshots = load_saved_view_raw_snapshots(database.personal(), &state.saved_views)
            .map_err(map_personal_error)?;
        project_legacy_saved_view_dynamic_compatibility(
            database.operational_mut(),
            &mut state.saved_views,
            &raw_snapshots,
        );
        drop(database);
        state.active_watch_count = u8::try_from(self.watch.total_active_watch_count())
            .unwrap_or(bcsp_contracts::MAX_ACTIVE_WATCHES);
        encode(&Bootstrap {
            mode: "LOCAL",
            session_nonce: nonce.as_str(),
            state,
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
        let current = self.with_store(|store| store.current_filters())?;
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

        let mut database = self.lock_database()?;
        let (current_filters, mut definitions, raw_snapshots) = database
            .personal()
            .consistent_read(|store| {
                let current_filters = store.current_filters()?;
                let definitions = store.saved_views()?;
                let raw_snapshots = load_saved_view_raw_snapshots(store, &definitions)?;
                Ok((current_filters, definitions, raw_snapshots))
            })
            .map_err(map_personal_error)?;
        project_legacy_saved_view_dynamic_compatibility(
            database.operational_mut(),
            &mut definitions,
            &raw_snapshots,
        );
        drop(database);
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
        let dynamically_incompatible = {
            let mut database = self.lock_database()?;
            let definition = {
                let store = database.personal();
                let state_revision = store.user_state_revision().map_err(map_personal_error)?;
                let current_filters = store.current_filters().map_err(map_personal_error)?;
                if state_revision != command.expected_user_state_revision
                    || current_filters.revision != command.expected_current_filters_revision
                {
                    None
                } else {
                    store.saved_view(command.id).map_err(map_personal_error)?
                }
            };
            definition.is_some_and(|definition| {
                definition.revision == command.expected_view_revision
                    && legacy_saved_view_has_invalid_dynamic_option(
                        database.operational_mut(),
                        &definition,
                    )
            })
        };
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

        self.watch.stop();
        self.watch.flush_dispatch_sink();
        let reset = mutation_store
            .clear_personal_data(expected_state_revision)
            .map_err(map_personal_error)?;
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
        if definition.schema_version != 1 || definition.content.filters().is_none() {
            continue;
        }
        if let Some((revision, raw)) = store.saved_view_raw_snapshot(definition.id)? {
            snapshots.push((definition.id, revision, raw));
        }
    }
    Ok(snapshots)
}

fn project_legacy_saved_view_dynamic_compatibility(
    storage: &mut OperationalStorage,
    definitions: &mut [SavedViewDefinition],
    raw_snapshots: &SavedViewRawSnapshots,
) {
    project_legacy_saved_view_dynamic_compatibility_with(
        definitions,
        raw_snapshots,
        |definition| legacy_saved_view_has_invalid_dynamic_option(storage, definition),
    );
}

fn project_legacy_saved_view_dynamic_compatibility_with(
    definitions: &mut [SavedViewDefinition],
    raw_snapshots: &SavedViewRawSnapshots,
    mut is_invalid: impl FnMut(&SavedViewDefinition) -> bool,
) {
    for definition in definitions {
        if !is_invalid(definition) {
            continue;
        }
        let Some((_, _, raw_snapshot)) = raw_snapshots
            .iter()
            .find(|(id, revision, _)| *id == definition.id && *revision == definition.revision)
        else {
            // A concurrent update can make a separately read raw snapshot
            // ambiguous. Leaving the view compatible for this response is
            // safer than attaching the wrong original snapshot; apply-time
            // validation and the next library read still fail closed.
            continue;
        };
        definition.content = SavedViewContent::Incompatible {
            raw_snapshot: raw_snapshot.clone(),
            reason: SavedViewIncompatibility::InvalidFieldData,
        };
    }
}

fn legacy_saved_view_has_invalid_dynamic_option(
    storage: &mut OperationalStorage,
    definition: &SavedViewDefinition,
) -> bool {
    if definition.schema_version != 1 {
        return false;
    }
    let Some(filters) = definition.content.filters() else {
        return false;
    };
    let values = filters.values();
    if values.keywords().is_none()
        && values.levels().is_empty()
        && values.instructors().is_empty()
        && values.meeting_locations().locations.is_empty()
        && values.exam_codes().is_empty()
    {
        return false;
    }
    let Some(targets) = saved_view_search_targets(storage, filters) else {
        // Missing discovery/publication is transient and cannot prove that a
        // migrated dictionary value is invalid.
        return false;
    };
    matches!(
        SharedQueryService::new(storage).validate_dynamic_filter_options(&targets, values),
        Err(SharedQueryError::InvalidFilterOption { .. })
    )
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
    fn invalid_legacy_dynamic_option_projects_original_snapshot_as_incompatible() {
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
            |_| true,
        );
        assert_eq!(
            definitions[0].content,
            SavedViewContent::Incompatible {
                raw_snapshot: raw,
                reason: SavedViewIncompatibility::InvalidFieldData,
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
            |_| true,
        );
        assert_eq!(
            raced[0], definition,
            "a raw snapshot from another revision must never be attached"
        );
    }
}
