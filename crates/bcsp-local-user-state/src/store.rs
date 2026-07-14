use std::collections::BTreeSet;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use bcsp_contracts::{OpenEpisodeId, OpenEpisodeState, SectionKey, TraceId, WatchNotificationMode};
use rusqlite::types::Value as SqlValue;
use rusqlite::{
    Connection, OptionalExtension, Transaction, TransactionBehavior, params, params_from_iter,
};

use crate::migration::{apply_migrations, read_migration_records};
use crate::{
    EpisodeActionInput, EpisodeActionKind, EpisodeActionRecord, EpisodeDisposition,
    EpisodeHistoryIdentity, EpisodeHistorySummary, EpisodeSummaryInput, HistoryFilter, HistoryPage,
    HistoryWriteOutcome, LocalSettings, MAX_SELECTED_SECTIONS, PageRequest,
    PersonalMigrationRecord, PersonalResetResult, PersonalStateError, PersonalStateResult,
    PersonalStateSnapshot, PersonalTableCounts, SelectionMutation, SettingsRevision,
    SqliteConfiguration, StoredSettings, UnixMillis, WalCheckpoint,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub struct PersonalStateStore {
    connection: Connection,
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
        expected_revision: SettingsRevision,
        next: &LocalSettings,
    ) -> PersonalStateResult<StoredSettings> {
        let _ = SettingsRevision::try_from(expected_revision.get())?;
        next.validate()?;
        let settings_json = serde_json::to_string(next)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
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
        sections: &[SectionKey],
    ) -> PersonalStateResult<()> {
        validate_selection(sections)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        replace_selection(&transaction, sections)?;
        transaction.commit()?;
        Ok(())
    }

    pub fn add_selected_section(
        &mut self,
        section: SectionKey,
    ) -> PersonalStateResult<SelectionMutation> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
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

    pub fn remove_selected_section(&mut self, section: &SectionKey) -> PersonalStateResult<bool> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
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

    pub fn snapshot(&self, page: PageRequest) -> PersonalStateResult<PersonalStateSnapshot> {
        Ok(PersonalStateSnapshot {
            settings: self.settings()?,
            selected_sections: self.selected_sections()?,
            episode_history: self.episode_history(&HistoryFilter::default(), page)?,
            active_watch_count: 0,
        })
    }

    pub fn personal_table_counts(&self) -> PersonalStateResult<PersonalTableCounts> {
        Ok(PersonalTableCounts {
            settings: table_count(&self.connection, "personal_settings_v1")?,
            selected_sections: table_count(&self.connection, "personal_selected_sections_v1")?,
            episode_summaries: table_count(&self.connection, "personal_episode_summaries_v1")?,
            episode_actions: table_count(&self.connection, "personal_episode_actions_v1")?,
        })
    }

    pub fn clear_personal_data(&mut self) -> PersonalStateResult<PersonalResetResult> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let deleted_episode_actions =
            transaction.execute("DELETE FROM personal_episode_actions_v1", [])?;
        let deleted_episode_summaries =
            transaction.execute("DELETE FROM personal_episode_summaries_v1", [])?;
        let deleted_selected_sections =
            transaction.execute("DELETE FROM personal_selected_sections_v1", [])?;
        let deleted_settings = transaction.execute("DELETE FROM personal_settings_v1", [])?;
        transaction.commit()?;
        Ok(PersonalResetResult {
            deleted_settings: deleted_settings as u64,
            deleted_selected_sections: deleted_selected_sections as u64,
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

fn table_count(connection: &Connection, table: &'static str) -> PersonalStateResult<u64> {
    let sql = match table {
        "personal_settings_v1" => "SELECT COUNT(*) FROM personal_settings_v1",
        "personal_selected_sections_v1" => "SELECT COUNT(*) FROM personal_selected_sections_v1",
        "personal_episode_summaries_v1" => "SELECT COUNT(*) FROM personal_episode_summaries_v1",
        "personal_episode_actions_v1" => "SELECT COUNT(*) FROM personal_episode_actions_v1",
        _ => return Err(PersonalStateError::SqliteConfiguration("table allowlist")),
    };
    let count = connection.query_row(sql, [], |row| row.get::<_, i64>(0))?;
    nonnegative_i64_to_u64(count)
}

fn u64_to_i64(value: u64) -> PersonalStateResult<i64> {
    i64::try_from(value).map_err(|_| PersonalStateError::StoredIntegerOutOfRange)
}

fn nonnegative_i64_to_u64(value: i64) -> PersonalStateResult<u64> {
    u64::try_from(value).map_err(|_| PersonalStateError::StoredIntegerOutOfRange)
}
