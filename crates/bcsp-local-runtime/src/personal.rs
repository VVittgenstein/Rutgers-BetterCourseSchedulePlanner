use std::sync::{Arc, Mutex};

use bcsp_application::SessionNonce;
use bcsp_application::SharedWatchSocket;
use bcsp_contracts::{
    ContractDecodeError, HttpRequestEnvelope, HttpSuccessEnvelope, SectionKey,
    decode_versioned_envelope_json,
};
use bcsp_local_user_state::{
    HistoryFilter, LocalSettings, PageRequest, PersonalStateError, SettingsRevision,
};
use serde::{Deserialize, Serialize};

use crate::{LocalApiErrorCode, LocalPrimaryDatabase, LocalSurfaceFailure, LocalSurfaceState};

pub struct PersonalSurface {
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    watch: Arc<SharedWatchSocket>,
}

impl PersonalSurface {
    pub fn new(database: Arc<Mutex<LocalPrimaryDatabase>>, watch: Arc<SharedWatchSocket>) -> Self {
        Self { database, watch }
    }

    fn with_store<T>(
        &self,
        operation: impl FnOnce(
            &mut bcsp_local_user_state::PersonalStateStore,
        ) -> Result<T, PersonalStateError>,
    ) -> Result<T, LocalSurfaceFailure> {
        let mut database = self
            .database
            .lock()
            .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))?;
        operation(database.personal_mut()).map_err(map_personal_error)
    }
}

impl LocalSurfaceState for PersonalSurface {
    fn bootstrap(&mut self, nonce: &SessionNonce) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Bootstrap<'a, T> {
            mode: &'static str,
            session_nonce: &'a str,
            state: T,
        }

        let mut state = self.with_store(|store| store.snapshot(PageRequest::DEFAULT))?;
        state.active_watch_count = u8::try_from(self.watch.total_active_watch_count())
            .unwrap_or(bcsp_contracts::MAX_ACTIVE_WATCHES);
        encode(&Bootstrap {
            mode: "LOCAL",
            session_nonce: nonce.as_str(),
            state,
        })
    }

    fn settings(&mut self) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let settings = self.with_store(|store| store.settings())?;
        encode(&settings)
    }

    fn put_settings(&mut self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Update {
            expected_revision: u64,
            value: LocalSettings,
        }

        let update: Update = decode_payload(body)?;
        let revision =
            SettingsRevision::try_from(update.expected_revision).map_err(map_personal_error)?;
        let stored =
            self.with_store(|store| store.compare_and_swap_settings(revision, &update.value))?;
        encode(&stored)
    }

    fn selection(&mut self) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let selection = self.with_store(|store| store.selected_sections())?;
        encode(&selection)
    }

    fn put_selection(&mut self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Update {
            sections: Vec<SectionKey>,
        }

        let update: Update = decode_payload(body)?;
        self.with_store(|store| store.replace_selected_sections(&update.sections))?;
        encode(&update.sections)
    }

    fn history(&mut self) -> Result<Vec<u8>, LocalSurfaceFailure> {
        let history = self.with_store(|store| {
            store.episode_history(&HistoryFilter::default(), PageRequest::DEFAULT)
        })?;
        encode(&history)
    }

    fn checkpoint_wal(&mut self) -> Result<(), LocalSurfaceFailure> {
        self.with_store(|store| store.checkpoint_wal()).map(|_| ())
    }
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
        PersonalStateError::RevisionConflict { .. } => {
            LocalSurfaceFailure::conflict(LocalApiErrorCode::SettingsRevisionConflict)
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
