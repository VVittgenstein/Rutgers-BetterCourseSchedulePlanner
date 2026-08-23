use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use bcsp_contracts::{
    OpenEpisodeId, OpenEpisodeState, SectionKey, TraceId, WatchNotificationMode, WatchPolicyV1,
};
use rusqlite::types::Value as SqlValue;
use rusqlite::{
    Connection, OptionalExtension, Transaction, TransactionBehavior, params, params_from_iter,
};
use sha2::{Digest, Sha256};

use crate::migration::{apply_migrations, read_migration_records};
use crate::{
    DesiredWatch, DesiredWatchAdmission, DesiredWatchAuthority, DesiredWatchCommand,
    DesiredWatchCommitted, DesiredWatchCounters, DesiredWatchEntry, DesiredWatchMutationOutcome,
    DesiredWatchReceipt, DesiredWatchSource, EpisodeActionInput, EpisodeActionKind,
    EpisodeActionRecord,
    EpisodeDisposition,
    EpisodeHistoryIdentity, EpisodeHistorySummary, EpisodeSummaryInput, HistoryFilter, HistoryPage,
    HistoryWriteOutcome, LocalSettings, MAX_DESIRED_WATCHES, MAX_SELECTED_SECTIONS, PageRequest,
    PersonalMigrationRecord, PersonalResetResult, PersonalStateError, PersonalStateResult,
    PersonalStateSnapshot, PersonalTableCounts, SelectionMutation, SettingsRevision,
    SqliteConfiguration, StoredSettings, UnixMillis, UserStateRevision, WalCheckpoint,
};

// A first-load Catalog publication can hold SQLite's single writer slot for
// tens of seconds on a clean low-resource Windows machine. Personal mutations
// run on blocking workers, so waiting here preserves the user action without
// starving the async HTTP runtime or returning a transient 500.
const BUSY_TIMEOUT: Duration = Duration::from_secs(120);
// Every authority counter crosses the wire to a JavaScript client, so the
// table bounds them here rather than at u64.
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub struct PersonalStateStore {
    pub(crate) connection: Connection,
}

impl PersonalStateStore {
    pub fn open(path: impl AsRef<Path>) -> PersonalStateResult<Self> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(BUSY_TIMEOUT)?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        apply_migrations(&mut connection)?;

        let store = Self { connection };
        let configuration = store.sqlite_configuration()?;
        if !configuration.foreign_keys {
            return Err(PersonalStateError::SqliteConfiguration("foreign_keys"));
        }
        if !configuration.journal_mode.eq_ignore_ascii_case("wal") {
            return Err(PersonalStateError::SqliteConfiguration("journal_mode"));
        }
        if configuration.busy_timeout_ms < BUSY_TIMEOUT.as_millis() as u64 {
            return Err(PersonalStateError::SqliteConfiguration("busy_timeout"));
        }
        Ok(store)
    }

    pub fn sqlite_configuration(&self) -> PersonalStateResult<SqliteConfiguration> {
        let journal_mode = self
            .connection
            .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))?;
        let foreign_keys = self
            .connection
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, bool>(0))?;
        let busy_timeout_ms = self
            .connection
            .pragma_query_value(None, "busy_timeout", |row| row.get::<_, i64>(0))?;
        Ok(SqliteConfiguration {
            journal_mode,
            foreign_keys,
            busy_timeout_ms: nonnegative_i64_to_u64(busy_timeout_ms)?,
        })
    }

    pub fn migration_records(&self) -> PersonalStateResult<Vec<PersonalMigrationRecord>> {
        read_migration_records(&self.connection)
    }

    pub fn settings(&self) -> PersonalStateResult<StoredSettings> {
        load_settings(&self.connection)
    }

    pub fn compare_and_swap_settings(
        &mut self,
        expected_state_revision: UserStateRevision,
        expected_revision: SettingsRevision,
        next: &LocalSettings,
    ) -> PersonalStateResult<StoredSettings> {
        let _ = SettingsRevision::try_from(expected_revision.get())?;
        next.validate()?;
        let settings_json = serde_json::to_string(next)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        let current = load_settings(&transaction)?;
        if current.revision != expected_revision {
            return Err(PersonalStateError::RevisionConflict {
                expected: expected_revision.get(),
                actual: current.revision.get(),
            });
        }
        let next_revision = current
            .revision
            .get()
            .checked_add(1)
            .ok_or(PersonalStateError::RevisionOverflow)?;
        let next_revision_sql = u64_to_i64(next_revision)?;
        transaction.execute(
            "INSERT INTO personal_settings_v1(singleton_id, revision, settings_json)
             VALUES (1, ?1, ?2)
             ON CONFLICT(singleton_id) DO UPDATE SET
                 revision = excluded.revision,
                 settings_json = excluded.settings_json",
            params![next_revision_sql, settings_json],
        )?;
        transaction.commit()?;
        Ok(StoredSettings {
            revision: SettingsRevision::from_stored(next_revision),
            value: next.clone(),
        })
    }

    pub fn selected_sections(&self) -> PersonalStateResult<Vec<SectionKey>> {
        load_selected_sections(&self.connection)
    }

    pub fn replace_selected_sections(
        &mut self,
        expected_state_revision: UserStateRevision,
        sections: &[SectionKey],
    ) -> PersonalStateResult<()> {
        validate_selection(sections)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        replace_selection(&transaction, sections)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn add_selected_section(
        &mut self,
        expected_state_revision: UserStateRevision,
        section: SectionKey,
    ) -> PersonalStateResult<SelectionMutation> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        let mut selected = load_selected_sections(&transaction)?;
        if let Some(position) = selected.iter().position(|value| value == &section) {
            return Ok(SelectionMutation::AlreadySelected {
                position: u8::try_from(position)
                    .map_err(|_| PersonalStateError::StoredIntegerOutOfRange)?,
            });
        }
        if selected.len() >= MAX_SELECTED_SECTIONS {
            return Err(PersonalStateError::SelectionLimitExceeded {
                maximum: MAX_SELECTED_SECTIONS,
            });
        }
        selected.push(section);
        replace_selection(&transaction, &selected)?;
        transaction.commit()?;
        Ok(SelectionMutation::Added {
            position: u8::try_from(selected.len() - 1)
                .map_err(|_| PersonalStateError::StoredIntegerOutOfRange)?,
        })
    }

    pub fn remove_selected_section(
        &mut self,
        expected_state_revision: UserStateRevision,
        section: &SectionKey,
    ) -> PersonalStateResult<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        let mut selected = load_selected_sections(&transaction)?;
        let original_len = selected.len();
        selected.retain(|value| value != section);
        if selected.len() == original_len {
            return Ok(false);
        }
        replace_selection(&transaction, &selected)?;
        transaction.commit()?;
        Ok(true)
    }

    /// The desired-watch intents a bootstrap should show, ordered by section
    /// key. Tombstones (`desired = 0`) are excluded, so this stays the exact
    /// shape protocol v1 already ships -- the authority state below is not on
    /// the wire until the frontend slice lands.
    pub fn desired_watches(&self) -> PersonalStateResult<Vec<DesiredWatch>> {
        load_desired_watches(&self.connection)
    }

    /// The full authority state: generation, counters, and every row
    /// INCLUDING tombstones.
    ///
    /// A tombstone is load-bearing rather than debris: a section whose row was
    /// deleted has no revision to compare a late `basedOnRevision` against, so
    /// a delayed START would be admitted and would resurrect intent the user
    /// had cancelled. Callers therefore see removals, not absences.
    pub fn desired_watch_authority(&self) -> PersonalStateResult<DesiredWatchAuthority> {
        // One WAL snapshot for both halves: counters read separately from rows
        // can straddle a concurrent Full Reset and report a generation that
        // never went with those rows.
        self.consistent_read(|store| {
            Ok(DesiredWatchAuthority {
                counters: load_desired_watch_counters(&store.connection)?,
                entries: load_desired_watch_entries(&store.connection)?,
            })
        })
    }

    /// Commits one compare-and-swap against a section's desired-watch row.
    ///
    /// The checks run in the frozen order, inside one IMMEDIATE transaction,
    /// because the order IS the contract: a caller whose generation is gone
    /// must not also be told its id collided, and a caller at the cap must
    /// not be told its revision was stale. Only the last step writes, so
    /// every refusal leaves the authority untouched.
    ///
    /// `admission` is consulted lazily, and only for a command that asks to
    /// watch and has already passed generation, receipt, revision, and the
    /// cap. A command that asks to STOP skips admission entirely: a section
    /// that has since become unsupported must still be stoppable.
    pub fn commit_desired_watch_mutation<A>(
        &mut self,
        command: &DesiredWatchCommand,
        admission: A,
    ) -> PersonalStateResult<DesiredWatchMutationOutcome>
    where
        A: FnOnce(&SectionKey) -> DesiredWatchAdmission,
    {
        let policy_json = command
            .policy
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let fingerprint = command_fingerprint(command, policy_json.as_deref());
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;

        // 1. Generation. A command from before a Full Reset is not stale in
        //    the revision sense -- the authority it spoke about no longer
        //    exists -- so it is refused without leaving a receipt behind.
        let counters = load_desired_watch_counters(&transaction)?;
        if command.authority_generation != counters.authority_generation {
            return Ok(DesiredWatchMutationOutcome::StaleGeneration {
                current: counters.authority_generation,
            });
        }

        // 2. Receipt. A repeated id replays what it produced the first time;
        //    a reused id carrying a different command is reported FROM the
        //    row that exists, never by inserting a second one.
        if command.source == DesiredWatchSource::User
            && let Some(receipt) = load_desired_watch_receipt(
                &transaction,
                counters.authority_generation,
                command.mutation_id,
            )?
        {
            return Ok(
                if receipt.section == command.section && receipt.fingerprint == fingerprint {
                    DesiredWatchMutationOutcome::AlreadyApplied(receipt.committed)
                } else {
                    DesiredWatchMutationOutcome::MutationIdConflict(receipt)
                },
            );
        }

        // 3. Revision. An absent row reads as 0, and a tombstone does not,
        //    which is the whole reason removals leave one behind.
        let existing = load_desired_watch_row(&transaction, &command.section)?;
        let current_revision = existing.as_ref().map_or(0, |row| row.revision);
        if command.based_on_revision != current_revision {
            return Ok(DesiredWatchMutationOutcome::StaleRevision {
                current: current_revision,
            });
        }

        // 4. Admission, for a command that asks to watch.
        if command.desired() {
            // A POST-STATE test, deliberately: only 0 -> 1 consumes a slot,
            // so editing the policy of one of nine armed sections still
            // commits. Testing "count < 9" instead would refuse it.
            let already_desired = existing.as_ref().is_some_and(|row| row.desired);
            if !already_desired && desired_watch_count(&transaction)? >= MAX_DESIRED_WATCHES as u64
            {
                return Ok(DesiredWatchMutationOutcome::LimitExceeded {
                    maximum: MAX_DESIRED_WATCHES,
                });
            }
            if let DesiredWatchAdmission::Reject(rejection) = admission(&command.section) {
                return Ok(DesiredWatchMutationOutcome::Rejected(rejection));
            }
        }

        // 5. Write. The revision always advances; the materialization epoch
        //    advances only when the `desired` VALUE changes, or when there is
        //    no previous epoch to keep.
        let revision = next_counter(counters.revision_counter)?;
        let (materialization_epoch, epoch_changed) = match &existing {
            Some(row) if row.desired == command.desired() => (row.materialization_epoch, false),
            _ => (next_counter(counters.materialization_counter)?, true),
        };
        transaction.execute(
            "INSERT INTO personal_desired_watches_v1(
                 term_id, campus_code, section_index, desired, policy_json,
                 revision, materialization_epoch
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(term_id, campus_code, section_index) DO UPDATE SET
                 desired = excluded.desired,
                 policy_json = excluded.policy_json,
                 revision = excluded.revision,
                 materialization_epoch = excluded.materialization_epoch",
            params![
                command.section.term().as_str(),
                command.section.campus().as_str(),
                command.section.index().as_str(),
                i64::from(command.desired()),
                policy_json,
                u64_to_i64(revision)?,
                u64_to_i64(materialization_epoch)?,
            ],
        )?;
        let materialization_counter = if epoch_changed {
            materialization_epoch
        } else {
            counters.materialization_counter
        };
        transaction.execute(
            "UPDATE personal_state_metadata_v1
                SET desired_watch_revision_counter = ?1,
                    desired_watch_materialization_counter = ?2
              WHERE singleton_id = 1",
            params![u64_to_i64(revision)?, u64_to_i64(materialization_counter)?],
        )?;
        let committed = DesiredWatchCommitted {
            revision,
            materialization_epoch,
            epoch_changed,
        };
        if command.source == DesiredWatchSource::User {
            transaction.execute(
                "INSERT INTO personal_desired_watch_receipts_v1(
                     authority_generation, mutation_id, term_id, campus_code,
                     section_index, fingerprint, outcome_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    u64_to_i64(counters.authority_generation)?,
                    command.mutation_id.to_string(),
                    command.section.term().as_str(),
                    command.section.campus().as_str(),
                    command.section.index().as_str(),
                    fingerprint,
                    serde_json::to_string(&committed)?,
                ],
            )?;
        }
        transaction.commit()?;
        Ok(DesiredWatchMutationOutcome::Committed(committed))
    }

    pub fn upsert_episode_summary(
        &mut self,
        summary: &EpisodeSummaryInput,
    ) -> PersonalStateResult<()> {
        summary.validate()?;
        let identity = &summary.identity;
        let disposition_json = summary
            .disposition
            .map(|disposition| serde_json::to_string(&disposition))
            .transpose()?;
        self.connection.execute(
            "INSERT INTO personal_episode_summaries_v1(
                term_id, campus_code, section_index, run_id, episode_id, state, mode,
                first_observed_at_ms, last_observed_at_ms, acknowledged_at_ms,
                timed_out_at_ms, closed_at_ms, disposition_json, audible_count,
                observation_count, last_observation_id
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16
             )
             ON CONFLICT(term_id, campus_code, section_index, run_id, episode_id)
             DO UPDATE SET
                state = excluded.state,
                mode = excluded.mode,
                first_observed_at_ms = excluded.first_observed_at_ms,
                last_observed_at_ms = excluded.last_observed_at_ms,
                acknowledged_at_ms = excluded.acknowledged_at_ms,
                timed_out_at_ms = excluded.timed_out_at_ms,
                closed_at_ms = excluded.closed_at_ms,
                disposition_json = excluded.disposition_json,
                audible_count = excluded.audible_count,
                observation_count = excluded.observation_count,
                last_observation_id = excluded.last_observation_id",
            params![
                identity.section_key.term().as_str(),
                identity.section_key.campus().as_str(),
                identity.section_key.index().as_str(),
                identity.run_id.to_string(),
                identity.episode_id.trace_id().to_string(),
                episode_state_wire(summary.state),
                notification_mode_wire(summary.mode),
                summary.first_observed_at.get(),
                summary.last_observed_at.get(),
                summary.acknowledged_at.map(UnixMillis::get),
                summary.timed_out_at.map(UnixMillis::get),
                summary.closed_at.map(UnixMillis::get),
                disposition_json,
                u64_to_i64(summary.audible_count)?,
                u64_to_i64(summary.observation_count.get())?,
                summary.last_observation_id.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn append_episode_action(
        &mut self,
        action: &EpisodeActionInput,
    ) -> PersonalStateResult<HistoryWriteOutcome> {
        if action.occurred_at.get() < 0 {
            return Err(crate::SettingValueError::NegativeTimestamp.into());
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = load_action_by_id(&transaction, action.action_id)? {
            if existing == *action {
                return Ok(HistoryWriteOutcome::AlreadyPresent);
            }
            return Err(PersonalStateError::ActionIdConflict);
        }
        if !episode_summary_exists(&transaction, &action.identity)? {
            return Err(PersonalStateError::EpisodeSummaryNotFound);
        }
        let identity = &action.identity;
        transaction.execute(
            "INSERT INTO personal_episode_actions_v1(
                action_id, term_id, campus_code, section_index, run_id, episode_id,
                action_kind, occurred_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                action.action_id.to_string(),
                identity.section_key.term().as_str(),
                identity.section_key.campus().as_str(),
                identity.section_key.index().as_str(),
                identity.run_id.to_string(),
                identity.episode_id.trace_id().to_string(),
                action.kind.wire_name(),
                action.occurred_at.get(),
            ],
        )?;
        transaction.commit()?;
        Ok(HistoryWriteOutcome::Inserted)
    }

    pub fn episode_history(
        &self,
        filter: &HistoryFilter,
        page: PageRequest,
    ) -> PersonalStateResult<HistoryPage<EpisodeHistorySummary>> {
        let page = PageRequest::try_new(page.offset, page.limit)?;
        let query = build_history_filter("s", filter, true);
        let count_sql = format!(
            "SELECT COUNT(*) FROM personal_episode_summaries_v1 AS s{}",
            query.where_sql
        );
        let count = self.connection.query_row(
            &count_sql,
            params_from_iter(query.params.iter()),
            |row| row.get::<_, i64>(0),
        )?;

        let data_sql = format!(
            "SELECT
                s.term_id, s.campus_code, s.section_index, s.run_id, s.episode_id,
                s.state, s.mode, s.first_observed_at_ms, s.last_observed_at_ms,
                s.acknowledged_at_ms, s.timed_out_at_ms, s.closed_at_ms,
                s.disposition_json, s.audible_count, s.observation_count,
                s.last_observation_id,
                (SELECT COUNT(*) FROM personal_episode_actions_v1 AS a
                 WHERE a.term_id = s.term_id AND a.campus_code = s.campus_code
                   AND a.section_index = s.section_index AND a.run_id = s.run_id
                   AND a.episode_id = s.episode_id) AS action_count
             FROM personal_episode_summaries_v1 AS s{}
             ORDER BY s.last_observed_at_ms DESC, s.term_id, s.campus_code,
                      s.section_index, s.run_id, s.episode_id
             LIMIT ? OFFSET ?",
            query.where_sql
        );
        let mut data_params = query.params;
        data_params.push(SqlValue::Integer(i64::from(page.limit)));
        data_params.push(SqlValue::Integer(
            i64::try_from(page.offset).map_err(|_| PersonalStateError::InvalidPageOffset)?,
        ));
        let mut statement = self.connection.prepare(&data_sql)?;
        let rows = statement.query_map(params_from_iter(data_params.iter()), |row| {
            Ok(RawSummary {
                term: row.get(0)?,
                campus: row.get(1)?,
                section_index: row.get(2)?,
                run_id: row.get(3)?,
                episode_id: row.get(4)?,
                state: row.get(5)?,
                mode: row.get(6)?,
                first_observed_at_ms: row.get(7)?,
                last_observed_at_ms: row.get(8)?,
                acknowledged_at_ms: row.get(9)?,
                timed_out_at_ms: row.get(10)?,
                closed_at_ms: row.get(11)?,
                disposition_json: row.get(12)?,
                audible_count: row.get(13)?,
                observation_count: row.get(14)?,
                last_observation_id: row.get(15)?,
                action_count: row.get(16)?,
            })
        })?;
        let raw = rows.collect::<Result<Vec<_>, _>>()?;
        let items = raw
            .into_iter()
            .map(parse_summary)
            .collect::<PersonalStateResult<Vec<_>>>()?;
        Ok(HistoryPage {
            items,
            total: nonnegative_i64_to_u64(count)?,
            offset: page.offset,
            limit: page.limit,
        })
    }

    pub fn episode_actions(
        &self,
        filter: &HistoryFilter,
        page: PageRequest,
    ) -> PersonalStateResult<HistoryPage<EpisodeActionRecord>> {
        let page = PageRequest::try_new(page.offset, page.limit)?;
        let query = build_history_filter("a", filter, false);
        let count_sql = format!(
            "SELECT COUNT(*) FROM personal_episode_actions_v1 AS a{}",
            query.where_sql
        );
        let count = self.connection.query_row(
            &count_sql,
            params_from_iter(query.params.iter()),
            |row| row.get::<_, i64>(0),
        )?;
        let data_sql = format!(
            "SELECT a.action_id, a.term_id, a.campus_code, a.section_index,
                    a.run_id, a.episode_id, a.action_kind, a.occurred_at_ms
             FROM personal_episode_actions_v1 AS a{}
             ORDER BY a.occurred_at_ms DESC, a.action_id
             LIMIT ? OFFSET ?",
            query.where_sql
        );
        let mut data_params = query.params;
        data_params.push(SqlValue::Integer(i64::from(page.limit)));
        data_params.push(SqlValue::Integer(
            i64::try_from(page.offset).map_err(|_| PersonalStateError::InvalidPageOffset)?,
        ));
        let mut statement = self.connection.prepare(&data_sql)?;
        let rows = statement.query_map(params_from_iter(data_params.iter()), |row| {
            Ok(RawAction {
                action_id: row.get(0)?,
                term: row.get(1)?,
                campus: row.get(2)?,
                section_index: row.get(3)?,
                run_id: row.get(4)?,
                episode_id: row.get(5)?,
                kind: row.get(6)?,
                occurred_at_ms: row.get(7)?,
            })
        })?;
        let raw = rows.collect::<Result<Vec<_>, _>>()?;
        let items = raw
            .into_iter()
            .map(parse_action)
            .collect::<PersonalStateResult<Vec<_>>>()?;
        Ok(HistoryPage {
            items,
            total: nonnegative_i64_to_u64(count)?,
            offset: page.offset,
            limit: page.limit,
        })
    }

    pub fn delete_episode_history(
        &mut self,
        identity: &EpisodeHistoryIdentity,
    ) -> PersonalStateResult<bool> {
        let changed = self.connection.execute(
            "DELETE FROM personal_episode_summaries_v1
             WHERE term_id = ?1 AND campus_code = ?2 AND section_index = ?3
               AND run_id = ?4 AND episode_id = ?5",
            params![
                identity.section_key.term().as_str(),
                identity.section_key.campus().as_str(),
                identity.section_key.index().as_str(),
                identity.run_id.to_string(),
                identity.episode_id.trace_id().to_string(),
            ],
        )?;
        Ok(changed != 0)
    }

    pub fn delete_episode_action(&mut self, action_id: TraceId) -> PersonalStateResult<bool> {
        Ok(self.connection.execute(
            "DELETE FROM personal_episode_actions_v1 WHERE action_id = ?1",
            [action_id.to_string()],
        )? != 0)
    }

    pub fn consistent_read<T>(
        &self,
        operation: impl FnOnce(&Self) -> PersonalStateResult<T>,
    ) -> PersonalStateResult<T> {
        let transaction = self.connection.unchecked_transaction()?;
        let value = operation(self)?;
        transaction.commit()?;
        Ok(value)
    }

    pub fn snapshot(&self, page: PageRequest) -> PersonalStateResult<PersonalStateSnapshot> {
        self.consistent_read(|store| {
            Ok(PersonalStateSnapshot {
                state_revision: store.user_state_revision()?,
                settings: store.settings()?,
                current_filters: store.current_filters()?,
                saved_views: store.saved_views()?,
                selected_sections: store.selected_sections()?,
                desired_watches: store.desired_watches()?,
                episode_history: store.episode_history(&HistoryFilter::default(), page)?,
                active_watch_count: 0,
            })
        })
    }

    /// Row counts for every allowlisted personal table.
    ///
    /// One statement, deliberately: eight separate `COUNT(*)`s can straddle a
    /// concurrent Full Reset and add up to a state that never existed, but
    /// wrapping them in a transaction here would break composition -- a caller
    /// that already holds [`Self::consistent_read`] would hit "cannot start a
    /// transaction within a transaction". A single SELECT gets the snapshot
    /// from the statement itself and nests safely.
    pub fn personal_table_counts(&self) -> PersonalStateResult<PersonalTableCounts> {
        let counts = self.connection.query_row(
            "SELECT
                 (SELECT COUNT(*) FROM personal_settings_v1),
                 (SELECT COUNT(*) FROM personal_current_filters_v1),
                 (SELECT COUNT(*) FROM personal_saved_views_v1),
                 (SELECT COUNT(*) FROM personal_selected_sections_v1),
                 (SELECT COUNT(*) FROM personal_desired_watches_v1),
                 (SELECT COUNT(*) FROM personal_desired_watch_receipts_v1),
                 (SELECT COUNT(*) FROM personal_episode_summaries_v1),
                 (SELECT COUNT(*) FROM personal_episode_actions_v1)",
            [],
            |row| {
                Ok([
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ])
            },
        )?;
        Ok(PersonalTableCounts {
            settings: nonnegative_i64_to_u64(counts[0])?,
            current_filters: nonnegative_i64_to_u64(counts[1])?,
            saved_views: nonnegative_i64_to_u64(counts[2])?,
            selected_sections: nonnegative_i64_to_u64(counts[3])?,
            desired_watches: nonnegative_i64_to_u64(counts[4])?,
            desired_watch_receipts: nonnegative_i64_to_u64(counts[5])?,
            episode_summaries: nonnegative_i64_to_u64(counts[6])?,
            episode_actions: nonnegative_i64_to_u64(counts[7])?,
        })
    }

    pub fn clear_personal_data(
        &mut self,
        expected_state_revision: UserStateRevision,
    ) -> PersonalStateResult<PersonalResetResult> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let actual_state_revision = transaction.query_row(
            "SELECT state_revision FROM personal_state_metadata_v1 WHERE singleton_id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let actual_state_revision = nonnegative_i64_to_u64(actual_state_revision)?;
        if actual_state_revision != expected_state_revision.get() {
            return Err(PersonalStateError::UserStateRevisionConflict {
                expected: expected_state_revision.get(),
                actual: actual_state_revision,
            });
        }
        let next_state_revision = actual_state_revision
            .checked_add(1)
            .ok_or(PersonalStateError::RevisionOverflow)?;
        let deleted_episode_actions =
            transaction.execute("DELETE FROM personal_episode_actions_v1", [])?;
        let deleted_episode_summaries =
            transaction.execute("DELETE FROM personal_episode_summaries_v1", [])?;
        let deleted_saved_views = transaction.execute("DELETE FROM personal_saved_views_v1", [])?;
        let deleted_current_filters =
            transaction.execute("DELETE FROM personal_current_filters_v1", [])?;
        let deleted_selected_sections =
            transaction.execute("DELETE FROM personal_selected_sections_v1", [])?;
        let deleted_desired_watches =
            transaction.execute("DELETE FROM personal_desired_watches_v1", [])?;
        let deleted_desired_watch_receipts =
            transaction.execute("DELETE FROM personal_desired_watch_receipts_v1", [])?;
        let deleted_settings = transaction.execute("DELETE FROM personal_settings_v1", [])?;
        // Full Reset is the authority's generation barrier: bump the
        // generation and zero the two counters in the SAME transaction that
        // deletes the rows. Without the bump, a command issued before the
        // reset still carries a matching generation and would be admitted
        // afterwards. The actor incarnation is deliberately untouched -- it
        // orders frames, not data.
        transaction.execute(
            "UPDATE personal_state_metadata_v1
                SET state_revision = ?1,
                    desired_watch_authority_generation =
                        desired_watch_authority_generation + 1,
                    desired_watch_revision_counter = 0,
                    desired_watch_materialization_counter = 0
              WHERE singleton_id = 1",
            [u64_to_i64(next_state_revision)?],
        )?;
        transaction.commit()?;
        Ok(PersonalResetResult {
            state_revision: UserStateRevision::from_stored(next_state_revision),
            deleted_settings: deleted_settings as u64,
            deleted_current_filters: deleted_current_filters as u64,
            deleted_saved_views: deleted_saved_views as u64,
            deleted_selected_sections: deleted_selected_sections as u64,
            deleted_desired_watches: deleted_desired_watches as u64,
            deleted_desired_watch_receipts: deleted_desired_watch_receipts as u64,
            deleted_episode_summaries: deleted_episode_summaries as u64,
            deleted_episode_actions: deleted_episode_actions as u64,
        })
    }

    pub fn checkpoint_wal(&self) -> PersonalStateResult<WalCheckpoint> {
        self.connection
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .map_err(Into::into)
            .and_then(|(busy, log_frames, checkpointed_frames)| {
                Ok(WalCheckpoint {
                    busy: nonnegative_i64_to_u64(busy)?,
                    log_frames: nonnegative_i64_to_u64(log_frames)?,
                    checkpointed_frames: nonnegative_i64_to_u64(checkpointed_frames)?,
                })
            })
    }
}

fn load_settings(connection: &Connection) -> PersonalStateResult<StoredSettings> {
    let row = connection
        .query_row(
            "SELECT revision, settings_json FROM personal_settings_v1 WHERE singleton_id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((revision, settings_json)) = row else {
        return Ok(StoredSettings {
            revision: SettingsRevision::ZERO,
            value: LocalSettings::default(),
        });
    };
    let revision = nonnegative_i64_to_u64(revision)?;
    let value = serde_json::from_str::<LocalSettings>(&settings_json)?;
    value.validate()?;
    Ok(StoredSettings {
        revision: SettingsRevision::from_stored(revision),
        value,
    })
}

fn require_user_state_revision(
    connection: &Connection,
    expected: UserStateRevision,
) -> PersonalStateResult<()> {
    let actual = connection.query_row(
        "SELECT state_revision FROM personal_state_metadata_v1 WHERE singleton_id = 1",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    let actual = nonnegative_i64_to_u64(actual)?;
    if actual != expected.get() {
        return Err(PersonalStateError::UserStateRevisionConflict {
            expected: expected.get(),
            actual,
        });
    }
    Ok(())
}

fn validate_selection(sections: &[SectionKey]) -> PersonalStateResult<()> {
    if sections.len() > MAX_SELECTED_SECTIONS {
        return Err(PersonalStateError::SelectionLimitExceeded {
            maximum: MAX_SELECTED_SECTIONS,
        });
    }
    let mut unique = BTreeSet::new();
    for section in sections {
        if !unique.insert(section.clone()) {
            return Err(PersonalStateError::DuplicateSelection(section.clone()));
        }
    }
    Ok(())
}

fn replace_selection(
    transaction: &Transaction<'_>,
    sections: &[SectionKey],
) -> PersonalStateResult<()> {
    transaction.execute("DELETE FROM personal_selected_sections_v1", [])?;
    let mut statement = transaction.prepare(
        "INSERT INTO personal_selected_sections_v1(
            position, term_id, campus_code, section_index
         ) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for (position, section) in sections.iter().enumerate() {
        statement.execute(params![
            i64::try_from(position).map_err(|_| PersonalStateError::StoredIntegerOutOfRange)?,
            section.term().as_str(),
            section.campus().as_str(),
            section.index().as_str(),
        ])?;
    }
    Ok(())
}

fn load_selected_sections(connection: &Connection) -> PersonalStateResult<Vec<SectionKey>> {
    let mut statement = connection.prepare(
        "SELECT term_id, campus_code, section_index
         FROM personal_selected_sections_v1
         ORDER BY position",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    rows.map(|row| {
        let (term, campus, section_index) = row?;
        SectionKey::try_new(&term, &campus, &section_index).map_err(Into::into)
    })
    .collect()
}

fn load_desired_watches(connection: &Connection) -> PersonalStateResult<Vec<DesiredWatch>> {
    let mut statement = connection.prepare(
        "SELECT term_id, campus_code, section_index, policy_json
         FROM personal_desired_watches_v1
         WHERE desired = 1
         ORDER BY term_id, campus_code, section_index",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    rows.map(|row| {
        let (term, campus, section_index, policy_json) = row?;
        Ok(DesiredWatch {
            section: SectionKey::try_new(&term, &campus, &section_index)?,
            policy: serde_json::from_str::<WatchPolicyV1>(&policy_json)?,
        })
    })
    .collect()
}

fn load_desired_watch_counters(
    connection: &Connection,
) -> PersonalStateResult<DesiredWatchCounters> {
    connection
        .query_row(
            "SELECT desired_watch_authority_generation,
                    desired_watch_revision_counter,
                    desired_watch_materialization_counter,
                    desired_watch_actor_incarnation
             FROM personal_state_metadata_v1
             WHERE singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .map_err(PersonalStateError::from)
        .and_then(|(generation, revision, materialization, incarnation)| {
            Ok(DesiredWatchCounters {
                authority_generation: nonnegative_i64_to_u64(generation)?,
                revision_counter: nonnegative_i64_to_u64(revision)?,
                materialization_counter: nonnegative_i64_to_u64(materialization)?,
                actor_incarnation: nonnegative_i64_to_u64(incarnation)?,
            })
        })
}

fn load_desired_watch_entries(
    connection: &Connection,
) -> PersonalStateResult<Vec<DesiredWatchEntry>> {
    let mut statement = connection.prepare(
        "SELECT term_id, campus_code, section_index, desired, policy_json,
                revision, materialization_epoch
         FROM personal_desired_watches_v1
         ORDER BY term_id, campus_code, section_index",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;
    rows.map(|row| {
        let (term, campus, index, desired, policy_json, revision, epoch) = row?;
        // The table CHECK already ties policy presence to `desired`; a
        // mismatch here would mean the file was edited outside this crate.
        let policy = match (desired, policy_json) {
            (1, Some(policy_json)) => Some(serde_json::from_str::<WatchPolicyV1>(&policy_json)?),
            (0, None) => None,
            _ => {
                return Err(PersonalStateError::InvalidStoredValue {
                    table: "personal_desired_watches_v1",
                    field: "policy_json",
                });
            }
        };
        Ok(DesiredWatchEntry {
            section: SectionKey::try_new(&term, &campus, &index)?,
            policy,
            revision: nonnegative_i64_to_u64(revision)?,
            materialization_epoch: nonnegative_i64_to_u64(epoch)?,
        })
    })
    .collect()
}

/// One desired-watch row as the CAS needs to see it.
struct DesiredWatchRow {
    desired: bool,
    revision: u64,
    materialization_epoch: u64,
}

fn load_desired_watch_row(
    connection: &Connection,
    section: &SectionKey,
) -> PersonalStateResult<Option<DesiredWatchRow>> {
    let row = connection
        .query_row(
            "SELECT desired, revision, materialization_epoch
             FROM personal_desired_watches_v1
             WHERE term_id = ?1 AND campus_code = ?2 AND section_index = ?3",
            params![
                section.term().as_str(),
                section.campus().as_str(),
                section.index().as_str(),
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    row.map(|(desired, revision, epoch)| {
        Ok(DesiredWatchRow {
            desired: desired != 0,
            revision: nonnegative_i64_to_u64(revision)?,
            materialization_epoch: nonnegative_i64_to_u64(epoch)?,
        })
    })
    .transpose()
}

/// Tombstones are excluded: they are revision history, not consumed slots.
fn desired_watch_count(connection: &Connection) -> PersonalStateResult<u64> {
    let count = connection.query_row(
        "SELECT COUNT(*) FROM personal_desired_watches_v1 WHERE desired = 1",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    nonnegative_i64_to_u64(count)
}

fn load_desired_watch_receipt(
    connection: &Connection,
    authority_generation: u64,
    mutation_id: TraceId,
) -> PersonalStateResult<Option<DesiredWatchReceipt>> {
    let row = connection
        .query_row(
            "SELECT term_id, campus_code, section_index, fingerprint, outcome_json
             FROM personal_desired_watch_receipts_v1
             WHERE authority_generation = ?1 AND mutation_id = ?2",
            params![u64_to_i64(authority_generation)?, mutation_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            },
        )
        .optional()?;
    row.map(|(term, campus, index, fingerprint, outcome_json)| {
        Ok(DesiredWatchReceipt {
            section: SectionKey::try_new(&term, &campus, &index)?,
            fingerprint,
            committed: serde_json::from_str::<DesiredWatchCommitted>(&outcome_json)?,
        })
    })
    .transpose()
}

/// A stable digest of everything a retry has to present identically. The
/// generation is not in it -- that is half the key -- and neither is the
/// source, because only user mutations are receipted at all. The NUL
/// separators keep the three section fields from running together.
fn command_fingerprint(command: &DesiredWatchCommand, policy_json: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(command.section.term().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(command.section.campus().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(command.section.index().as_str().as_bytes());
    hasher.update([0]);
    hasher.update(command.based_on_revision.to_be_bytes());
    hasher.update([u8::from(command.desired())]);
    hasher.update(policy_json.unwrap_or("").as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Advances one authority counter. Exhaustion is a real end state rather
/// than something to wrap silently: these values are compared as JavaScript
/// numbers on the other side of the wire.
fn next_counter(current: u64) -> PersonalStateResult<u64> {
    let next = current
        .checked_add(1)
        .filter(|next| *next <= MAX_SAFE_INTEGER)
        .ok_or(PersonalStateError::DesiredWatchCounterExhausted)?;
    Ok(next)
}

fn episode_summary_exists(
    connection: &Connection,
    identity: &EpisodeHistoryIdentity,
) -> PersonalStateResult<bool> {
    connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM personal_episode_summaries_v1
                WHERE term_id = ?1 AND campus_code = ?2 AND section_index = ?3
                  AND run_id = ?4 AND episode_id = ?5
             )",
            params![
                identity.section_key.term().as_str(),
                identity.section_key.campus().as_str(),
                identity.section_key.index().as_str(),
                identity.run_id.to_string(),
                identity.episode_id.trace_id().to_string(),
            ],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn load_action_by_id(
    connection: &Connection,
    action_id: TraceId,
) -> PersonalStateResult<Option<EpisodeActionRecord>> {
    let raw = connection
        .query_row(
            "SELECT action_id, term_id, campus_code, section_index, run_id, episode_id,
                    action_kind, occurred_at_ms
             FROM personal_episode_actions_v1 WHERE action_id = ?1",
            [action_id.to_string()],
            |row| {
                Ok(RawAction {
                    action_id: row.get(0)?,
                    term: row.get(1)?,
                    campus: row.get(2)?,
                    section_index: row.get(3)?,
                    run_id: row.get(4)?,
                    episode_id: row.get(5)?,
                    kind: row.get(6)?,
                    occurred_at_ms: row.get(7)?,
                })
            },
        )
        .optional()?;
    raw.map(parse_action).transpose()
}

struct HistoryQuery {
    where_sql: String,
    params: Vec<SqlValue>,
}

fn build_history_filter(alias: &str, filter: &HistoryFilter, summaries: bool) -> HistoryQuery {
    let mut clauses = Vec::new();
    let mut params = Vec::new();
    if let Some(section) = &filter.section_key {
        clauses.push(format!(
            "{alias}.term_id = ? AND {alias}.campus_code = ? AND {alias}.section_index = ?"
        ));
        params.push(SqlValue::Text(section.term().as_str().to_owned()));
        params.push(SqlValue::Text(section.campus().as_str().to_owned()));
        params.push(SqlValue::Text(section.index().as_str().to_owned()));
    }
    if let Some(run_id) = filter.run_id {
        clauses.push(format!("{alias}.run_id = ?"));
        params.push(SqlValue::Text(run_id.to_string()));
    }
    if let Some(episode_id) = filter.episode_id {
        clauses.push(format!("{alias}.episode_id = ?"));
        params.push(SqlValue::Text(episode_id.trace_id().to_string()));
    }
    if let Some(kind) = filter.action_kind {
        if summaries {
            clauses.push(format!(
                "EXISTS (SELECT 1 FROM personal_episode_actions_v1 AS filtered_action
                 WHERE filtered_action.term_id = {alias}.term_id
                   AND filtered_action.campus_code = {alias}.campus_code
                   AND filtered_action.section_index = {alias}.section_index
                   AND filtered_action.run_id = {alias}.run_id
                   AND filtered_action.episode_id = {alias}.episode_id
                   AND filtered_action.action_kind = ?)"
            ));
        } else {
            clauses.push(format!("{alias}.action_kind = ?"));
        }
        params.push(SqlValue::Text(kind.wire_name().to_owned()));
    }
    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    HistoryQuery { where_sql, params }
}

struct RawSummary {
    term: String,
    campus: String,
    section_index: String,
    run_id: String,
    episode_id: String,
    state: String,
    mode: String,
    first_observed_at_ms: i64,
    last_observed_at_ms: i64,
    acknowledged_at_ms: Option<i64>,
    timed_out_at_ms: Option<i64>,
    closed_at_ms: Option<i64>,
    disposition_json: Option<String>,
    audible_count: i64,
    observation_count: i64,
    last_observation_id: String,
    action_count: i64,
}

fn parse_summary(raw: RawSummary) -> PersonalStateResult<EpisodeHistorySummary> {
    let observation_count = std::num::NonZeroU64::new(nonnegative_i64_to_u64(
        raw.observation_count,
    )?)
    .ok_or(PersonalStateError::InvalidStoredValue {
        table: "personal_episode_summaries_v1",
        field: "observation_count",
    })?;
    let summary = EpisodeHistorySummary {
        identity: parse_identity(
            raw.term,
            raw.campus,
            raw.section_index,
            raw.run_id,
            raw.episode_id,
        )?,
        state: parse_episode_state(&raw.state)?,
        mode: parse_notification_mode(&raw.mode)?,
        first_observed_at: UnixMillis::try_from(raw.first_observed_at_ms)?,
        last_observed_at: UnixMillis::try_from(raw.last_observed_at_ms)?,
        acknowledged_at: raw
            .acknowledged_at_ms
            .map(UnixMillis::try_from)
            .transpose()?,
        timed_out_at: raw.timed_out_at_ms.map(UnixMillis::try_from).transpose()?,
        closed_at: raw.closed_at_ms.map(UnixMillis::try_from).transpose()?,
        disposition: raw
            .disposition_json
            .map(|value| serde_json::from_str::<EpisodeDisposition>(&value))
            .transpose()?,
        audible_count: nonnegative_i64_to_u64(raw.audible_count)?,
        observation_count,
        last_observation_id: TraceId::from_str(&raw.last_observation_id)?,
        action_count: nonnegative_i64_to_u64(raw.action_count)?,
    };
    EpisodeSummaryInput {
        identity: summary.identity.clone(),
        state: summary.state,
        mode: summary.mode,
        first_observed_at: summary.first_observed_at,
        last_observed_at: summary.last_observed_at,
        acknowledged_at: summary.acknowledged_at,
        timed_out_at: summary.timed_out_at,
        closed_at: summary.closed_at,
        disposition: summary.disposition,
        audible_count: summary.audible_count,
        observation_count: summary.observation_count,
        last_observation_id: summary.last_observation_id,
    }
    .validate()?;
    Ok(summary)
}

struct RawAction {
    action_id: String,
    term: String,
    campus: String,
    section_index: String,
    run_id: String,
    episode_id: String,
    kind: String,
    occurred_at_ms: i64,
}

fn parse_action(raw: RawAction) -> PersonalStateResult<EpisodeActionRecord> {
    let kind =
        EpisodeActionKind::from_wire(&raw.kind).ok_or(PersonalStateError::InvalidStoredValue {
            table: "personal_episode_actions_v1",
            field: "action_kind",
        })?;
    Ok(EpisodeActionRecord {
        action_id: TraceId::from_str(&raw.action_id)?,
        identity: parse_identity(
            raw.term,
            raw.campus,
            raw.section_index,
            raw.run_id,
            raw.episode_id,
        )?,
        kind,
        occurred_at: UnixMillis::try_from(raw.occurred_at_ms)?,
    })
}

fn parse_identity(
    term: String,
    campus: String,
    section_index: String,
    run_id: String,
    episode_id: String,
) -> PersonalStateResult<EpisodeHistoryIdentity> {
    Ok(EpisodeHistoryIdentity::new(
        SectionKey::try_new(&term, &campus, &section_index)?,
        TraceId::from_str(&run_id)?,
        OpenEpisodeId::from(TraceId::from_str(&episode_id)?),
    ))
}

fn episode_state_wire(state: OpenEpisodeState) -> &'static str {
    match state {
        OpenEpisodeState::Unacknowledged => "UNACKNOWLEDGED",
        OpenEpisodeState::Acknowledged => "ACKNOWLEDGED",
        OpenEpisodeState::TimedOut => "TIMED_OUT",
        OpenEpisodeState::Closed => "CLOSED",
    }
}

fn parse_episode_state(value: &str) -> PersonalStateResult<OpenEpisodeState> {
    match value {
        "UNACKNOWLEDGED" => Ok(OpenEpisodeState::Unacknowledged),
        "ACKNOWLEDGED" => Ok(OpenEpisodeState::Acknowledged),
        "TIMED_OUT" => Ok(OpenEpisodeState::TimedOut),
        "CLOSED" => Ok(OpenEpisodeState::Closed),
        _ => Err(PersonalStateError::InvalidStoredValue {
            table: "personal_episode_summaries_v1",
            field: "state",
        }),
    }
}

fn notification_mode_wire(mode: WatchNotificationMode) -> &'static str {
    match mode {
        WatchNotificationMode::OneShot => "ONE_SHOT",
        WatchNotificationMode::Continuous => "CONTINUOUS",
    }
}

fn parse_notification_mode(value: &str) -> PersonalStateResult<WatchNotificationMode> {
    match value {
        "ONE_SHOT" => Ok(WatchNotificationMode::OneShot),
        "CONTINUOUS" => Ok(WatchNotificationMode::Continuous),
        _ => Err(PersonalStateError::InvalidStoredValue {
            table: "personal_episode_summaries_v1",
            field: "mode",
        }),
    }
}

fn u64_to_i64(value: u64) -> PersonalStateResult<i64> {
    i64::try_from(value).map_err(|_| PersonalStateError::StoredIntegerOutOfRange)
}

fn nonnegative_i64_to_u64(value: i64) -> PersonalStateResult<u64> {
    u64::try_from(value).map_err(|_| PersonalStateError::StoredIntegerOutOfRange)
}
