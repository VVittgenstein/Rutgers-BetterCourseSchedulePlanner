use std::str::FromStr;

use bcsp_contracts::{FilterRequestV1, SystemTraceIdSource, TraceId, TraceIdSource};
use rusqlite::{
    Connection, Error as SqliteError, OptionalExtension, Transaction, TransactionBehavior, params,
};
use serde_json::Value as JsonValue;

use crate::saved_view::{decode_filter_snapshot, encode_filter_snapshot};
use crate::{
    CurrentFilters, CurrentFiltersRevision, FilterAssociation, PersonalStateError,
    PersonalStateResult, PersonalStateStore, SavedViewContent, SavedViewDefinition,
    SavedViewDeleteResult, SavedViewMutation, SavedViewRevision, SavedViewsDeleteAllResult,
    StoredCurrentFilters, UnixMillis, UserStateRevision,
};

impl PersonalStateStore {
    pub fn user_state_revision(&self) -> PersonalStateResult<UserStateRevision> {
        load_user_state_revision(&self.connection)
    }

    pub fn current_filters(&self) -> PersonalStateResult<StoredCurrentFilters> {
        load_current_filters(&self.connection)
    }

    pub fn saved_views(&self) -> PersonalStateResult<Vec<SavedViewDefinition>> {
        let mut statement = self.connection.prepare(
            "SELECT view_id, name, schema_version, revision, snapshot_json,
                    created_at_ms, updated_at_ms
             FROM personal_saved_views_v1
             ORDER BY name_key, view_id",
        )?;
        let rows = statement.query_map([], raw_saved_view)?;
        rows.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(parse_saved_view)
            .collect()
    }

    pub fn saved_view(&self, id: TraceId) -> PersonalStateResult<Option<SavedViewDefinition>> {
        load_saved_view(&self.connection, id)
    }

    pub fn create_saved_view(
        &mut self,
        expected_state_revision: UserStateRevision,
        expected_current_revision: CurrentFiltersRevision,
        name: &str,
        filters: &FilterRequestV1,
        now: UnixMillis,
    ) -> PersonalStateResult<SavedViewMutation> {
        let (name, name_key) = normalize_saved_view_name(name)?;
        let encoded = encode_filter_snapshot(filters)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        require_current_revision(&transaction, expected_current_revision)?;
        require_name_available(&transaction, &name_key, None)?;
        let id = next_saved_view_id(&transaction)?;
        transaction
            .execute(
                "INSERT INTO personal_saved_views_v1(
                    view_id, name, name_key, schema_version, revision, snapshot_json,
                    created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?6)",
                params![
                    id.to_string(),
                    name,
                    name_key,
                    u64_to_i64(encoded.schema_version)?,
                    serde_json::to_string(&encoded.raw)?,
                    now.get(),
                ],
            )
            .map_err(map_sqlite)?;
        let revision = SavedViewRevision::from_stored(1);
        let current_filters = write_current_filters(
            &transaction,
            expected_current_revision,
            Some(CurrentFilters::new(
                FilterAssociation::Applied {
                    view_id: id,
                    revision,
                },
                filters.clone(),
            )),
        )?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(SavedViewMutation {
            state_revision: current_filters.state_revision,
            definition: SavedViewDefinition {
                id,
                name,
                schema_version: encoded.schema_version,
                revision,
                content: SavedViewContent::Compatible {
                    filters: Box::new(filters.clone()),
                },
                created_at: now,
                updated_at: now,
            },
            current_filters,
        })
    }

    pub fn apply_saved_view(
        &mut self,
        expected_state_revision: UserStateRevision,
        expected_current_revision: CurrentFiltersRevision,
        id: TraceId,
        expected_view_revision: SavedViewRevision,
    ) -> PersonalStateResult<StoredCurrentFilters> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        require_current_revision(&transaction, expected_current_revision)?;
        let definition = require_saved_view(&transaction, id, expected_view_revision)?;
        let filters = definition.content.filters().cloned().ok_or_else(|| {
            PersonalStateError::SavedViewIncompatible {
                id,
                reason: definition
                    .content
                    .incompatibility()
                    .expect("incompatible content has a reason")
                    .clone(),
            }
        })?;
        let current = write_current_filters(
            &transaction,
            expected_current_revision,
            Some(CurrentFilters::new(
                FilterAssociation::Applied {
                    view_id: id,
                    revision: definition.revision,
                },
                filters,
            )),
        )?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(current)
    }

    pub fn rename_saved_view(
        &mut self,
        expected_state_revision: UserStateRevision,
        expected_current_revision: CurrentFiltersRevision,
        id: TraceId,
        expected_view_revision: SavedViewRevision,
        name: &str,
        now: UnixMillis,
    ) -> PersonalStateResult<SavedViewMutation> {
        let (name, name_key) = normalize_saved_view_name(name)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        let current = require_current_revision(&transaction, expected_current_revision)?;
        let mut definition = require_saved_view(&transaction, id, expected_view_revision)?;
        require_name_available(&transaction, &name_key, Some(id))?;
        let next_revision = next_saved_view_revision(definition.revision)?;
        let updated_at = now.max(definition.created_at);
        transaction
            .execute(
                "UPDATE personal_saved_views_v1
                 SET name = ?1, name_key = ?2, revision = ?3, updated_at_ms = ?4
                 WHERE view_id = ?5 AND revision = ?6",
                params![
                    name,
                    name_key,
                    u64_to_i64(next_revision.get())?,
                    updated_at.get(),
                    id.to_string(),
                    u64_to_i64(expected_view_revision.get())?,
                ],
            )
            .map_err(map_sqlite)?;
        let current_filters =
            refresh_association_revision(&transaction, current, id, next_revision)?;
        transaction.commit().map_err(map_sqlite)?;
        definition.name = name;
        definition.revision = next_revision;
        definition.updated_at = updated_at;
        Ok(SavedViewMutation {
            state_revision: current_filters.state_revision,
            definition,
            current_filters,
        })
    }

    pub fn update_saved_view(
        &mut self,
        expected_state_revision: UserStateRevision,
        expected_current_revision: CurrentFiltersRevision,
        id: TraceId,
        expected_view_revision: SavedViewRevision,
        filters: &FilterRequestV1,
        now: UnixMillis,
    ) -> PersonalStateResult<SavedViewMutation> {
        let encoded = encode_filter_snapshot(filters)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        require_current_revision(&transaction, expected_current_revision)?;
        let mut definition = require_saved_view(&transaction, id, expected_view_revision)?;
        let next_revision = next_saved_view_revision(definition.revision)?;
        let updated_at = now.max(definition.created_at);
        transaction
            .execute(
                "UPDATE personal_saved_views_v1
                 SET schema_version = ?1, snapshot_json = ?2, revision = ?3, updated_at_ms = ?4
                 WHERE view_id = ?5 AND revision = ?6",
                params![
                    u64_to_i64(encoded.schema_version)?,
                    serde_json::to_string(&encoded.raw)?,
                    u64_to_i64(next_revision.get())?,
                    updated_at.get(),
                    id.to_string(),
                    u64_to_i64(expected_view_revision.get())?,
                ],
            )
            .map_err(map_sqlite)?;
        let current_filters = write_current_filters(
            &transaction,
            expected_current_revision,
            Some(CurrentFilters::new(
                FilterAssociation::Applied {
                    view_id: id,
                    revision: next_revision,
                },
                filters.clone(),
            )),
        )?;
        transaction.commit().map_err(map_sqlite)?;
        definition.schema_version = encoded.schema_version;
        definition.revision = next_revision;
        definition.content = SavedViewContent::Compatible {
            filters: Box::new(filters.clone()),
        };
        definition.updated_at = updated_at;
        Ok(SavedViewMutation {
            state_revision: current_filters.state_revision,
            definition,
            current_filters,
        })
    }

    pub fn duplicate_saved_view(
        &mut self,
        expected_state_revision: UserStateRevision,
        id: TraceId,
        expected_view_revision: SavedViewRevision,
        name: &str,
        now: UnixMillis,
    ) -> PersonalStateResult<SavedViewMutation> {
        let (name, name_key) = normalize_saved_view_name(name)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        let current_filters = load_current_filters(&transaction)?;
        let source = require_saved_view(&transaction, id, expected_view_revision)?;
        require_name_available(&transaction, &name_key, None)?;
        let new_id = next_saved_view_id(&transaction)?;
        let raw = raw_snapshot(&transaction, id)?;
        transaction
            .execute(
                "INSERT INTO personal_saved_views_v1(
                    view_id, name, name_key, schema_version, revision, snapshot_json,
                    created_at_ms, updated_at_ms
                 ) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?6)",
                params![
                    new_id.to_string(),
                    name,
                    name_key,
                    u64_to_i64(source.schema_version)?,
                    serde_json::to_string(&raw)?,
                    now.get(),
                ],
            )
            .map_err(map_sqlite)?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(SavedViewMutation {
            state_revision: current_filters.state_revision,
            definition: SavedViewDefinition {
                id: new_id,
                name,
                schema_version: source.schema_version,
                revision: SavedViewRevision::from_stored(1),
                content: decode_filter_snapshot(raw, source.schema_version),
                created_at: now,
                updated_at: now,
            },
            current_filters,
        })
    }

    pub fn delete_saved_view(
        &mut self,
        expected_state_revision: UserStateRevision,
        expected_current_revision: CurrentFiltersRevision,
        id: TraceId,
        expected_view_revision: SavedViewRevision,
    ) -> PersonalStateResult<SavedViewDeleteResult> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        let current = require_current_revision(&transaction, expected_current_revision)?;
        let _ = require_saved_view(&transaction, id, expected_view_revision)?;
        transaction
            .execute(
                "DELETE FROM personal_saved_views_v1 WHERE view_id = ?1 AND revision = ?2",
                params![id.to_string(), u64_to_i64(expected_view_revision.get())?],
            )
            .map_err(map_sqlite)?;
        let current_filters = detach_association(&transaction, current, Some(id))?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(SavedViewDeleteResult {
            state_revision: current_filters.state_revision,
            deleted_id: id,
            current_filters,
        })
    }

    pub fn delete_all_saved_views(
        &mut self,
        expected_state_revision: UserStateRevision,
        expected_current_revision: CurrentFiltersRevision,
    ) -> PersonalStateResult<SavedViewsDeleteAllResult> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        let current = require_current_revision(&transaction, expected_current_revision)?;
        let deleted = transaction
            .execute("DELETE FROM personal_saved_views_v1", [])
            .map_err(map_sqlite)?;
        let current_filters = detach_association(&transaction, current, None)?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(SavedViewsDeleteAllResult {
            state_revision: current_filters.state_revision,
            deleted_views: deleted as u64,
            current_filters,
        })
    }

    pub fn reset_current_filters(
        &mut self,
        expected_state_revision: UserStateRevision,
        expected_current_revision: CurrentFiltersRevision,
        defaults: Option<&FilterRequestV1>,
    ) -> PersonalStateResult<StoredCurrentFilters> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        require_current_revision(&transaction, expected_current_revision)?;
        let value = defaults
            .cloned()
            .map(|filters| CurrentFilters::new(FilterAssociation::Custom, filters));
        let current = write_current_filters(&transaction, expected_current_revision, value)?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(current)
    }

    pub fn replace_current_filters(
        &mut self,
        expected_state_revision: UserStateRevision,
        expected_current_revision: CurrentFiltersRevision,
        filters: &FilterRequestV1,
    ) -> PersonalStateResult<StoredCurrentFilters> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sqlite)?;
        require_user_state_revision(&transaction, expected_state_revision)?;
        let current = require_current_revision(&transaction, expected_current_revision)?;
        let association = current
            .value
            .as_ref()
            .map_or(FilterAssociation::Custom, |value| value.association.clone());
        let next = write_current_filters(
            &transaction,
            expected_current_revision,
            Some(CurrentFilters::new(association, filters.clone())),
        )?;
        transaction.commit().map_err(map_sqlite)?;
        Ok(next)
    }
}

fn load_user_state_revision(connection: &Connection) -> PersonalStateResult<UserStateRevision> {
    let revision = connection.query_row(
        "SELECT state_revision FROM personal_state_metadata_v1 WHERE singleton_id = 1",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    let revision = nonnegative_i64_to_u64(revision)?;
    if revision == 0 {
        return Err(PersonalStateError::InvalidStoredValue {
            table: "personal_state_metadata_v1",
            field: "state_revision",
        });
    }
    Ok(UserStateRevision::from_stored(revision))
}

fn require_user_state_revision(
    connection: &Connection,
    expected: UserStateRevision,
) -> PersonalStateResult<()> {
    let actual = load_user_state_revision(connection)?;
    if actual != expected {
        return Err(PersonalStateError::UserStateRevisionConflict {
            expected: expected.get(),
            actual: actual.get(),
        });
    }
    Ok(())
}

fn load_current_filters(connection: &Connection) -> PersonalStateResult<StoredCurrentFilters> {
    let row = connection
        .query_row(
            "SELECT revision, has_value, association_json, schema_version, snapshot_json
             FROM personal_current_filters_v1 WHERE singleton_id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;
    let Some((revision, has_value, association, schema_version, snapshot)) = row else {
        return Ok(StoredCurrentFilters {
            state_revision: load_user_state_revision(connection)?,
            revision: CurrentFiltersRevision::ZERO,
            value: None,
        });
    };
    let revision = nonnegative_i64_to_u64(revision)?;
    if revision == 0 {
        return Err(PersonalStateError::InvalidStoredValue {
            table: "personal_current_filters_v1",
            field: "revision",
        });
    }
    let value = if has_value {
        let association = serde_json::from_str(
            association
                .as_deref()
                .ok_or(PersonalStateError::InvalidFilterSnapshot)?,
        )?;
        let schema_version = nonnegative_i64_to_u64(
            schema_version.ok_or(PersonalStateError::InvalidFilterSnapshot)?,
        )?;
        let raw = serde_json::from_str::<JsonValue>(
            snapshot
                .as_deref()
                .ok_or(PersonalStateError::InvalidFilterSnapshot)?,
        )?;
        Some(CurrentFilters {
            association,
            content: decode_filter_snapshot(raw, schema_version),
        })
    } else {
        None
    };
    Ok(StoredCurrentFilters {
        state_revision: load_user_state_revision(connection)?,
        revision: CurrentFiltersRevision::from_stored(revision),
        value,
    })
}

fn require_current_revision(
    connection: &Connection,
    expected: CurrentFiltersRevision,
) -> PersonalStateResult<StoredCurrentFilters> {
    let current = load_current_filters(connection)?;
    if current.revision != expected {
        return Err(PersonalStateError::CurrentFiltersRevisionConflict {
            expected: expected.get(),
            actual: current.revision.get(),
        });
    }
    Ok(current)
}

fn write_current_filters(
    transaction: &Transaction<'_>,
    expected: CurrentFiltersRevision,
    value: Option<CurrentFilters>,
) -> PersonalStateResult<StoredCurrentFilters> {
    let current = require_current_revision(transaction, expected)?;
    let next = next_current_revision(current.revision)?;
    let (has_value, association, schema_version, snapshot) = match &value {
        Some(current) => {
            let filters = current
                .filters()
                .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
            let encoded = encode_filter_snapshot(filters)?;
            (
                true,
                Some(serde_json::to_string(&current.association)?),
                Some(u64_to_i64(encoded.schema_version)?),
                Some(serde_json::to_string(&encoded.raw)?),
            )
        }
        None => (false, None, None, None),
    };
    transaction
        .execute(
            "INSERT INTO personal_current_filters_v1(
                singleton_id, revision, has_value, association_json, schema_version, snapshot_json
             ) VALUES (1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(singleton_id) DO UPDATE SET
                revision = excluded.revision,
                has_value = excluded.has_value,
                association_json = excluded.association_json,
                schema_version = excluded.schema_version,
                snapshot_json = excluded.snapshot_json",
            params![
                u64_to_i64(next.get())?,
                has_value,
                association,
                schema_version,
                snapshot,
            ],
        )
        .map_err(map_sqlite)?;
    Ok(StoredCurrentFilters {
        state_revision: current.state_revision,
        revision: next,
        value,
    })
}

fn refresh_association_revision(
    transaction: &Transaction<'_>,
    current: StoredCurrentFilters,
    id: TraceId,
    revision: SavedViewRevision,
) -> PersonalStateResult<StoredCurrentFilters> {
    let associated = current.value.as_ref().is_some_and(|value| {
        matches!(
            &value.association,
            FilterAssociation::Applied { view_id, .. } if *view_id == id
        )
    });
    if !associated {
        return Ok(current);
    }
    let next = next_current_revision(current.revision)?;
    let association = FilterAssociation::Applied {
        view_id: id,
        revision,
    };
    transaction
        .execute(
            "UPDATE personal_current_filters_v1
             SET revision = ?1, association_json = ?2
             WHERE singleton_id = 1 AND revision = ?3",
            params![
                u64_to_i64(next.get())?,
                serde_json::to_string(&association)?,
                u64_to_i64(current.revision.get())?,
            ],
        )
        .map_err(map_sqlite)?;
    let mut value = current.value.expect("associated current filters exist");
    value.association = association;
    Ok(StoredCurrentFilters {
        state_revision: current.state_revision,
        revision: next,
        value: Some(value),
    })
}

fn detach_association(
    transaction: &Transaction<'_>,
    current: StoredCurrentFilters,
    only_id: Option<TraceId>,
) -> PersonalStateResult<StoredCurrentFilters> {
    let should_detach = current
        .value
        .as_ref()
        .is_some_and(|value| match &value.association {
            FilterAssociation::Applied { view_id, .. } => only_id.is_none_or(|id| id == *view_id),
            FilterAssociation::Custom => false,
        });
    if !should_detach {
        return Ok(current);
    }
    let next = next_current_revision(current.revision)?;
    transaction
        .execute(
            "UPDATE personal_current_filters_v1
             SET revision = ?1, association_json = ?2
             WHERE singleton_id = 1 AND revision = ?3",
            params![
                u64_to_i64(next.get())?,
                serde_json::to_string(&FilterAssociation::Custom)?,
                u64_to_i64(current.revision.get())?,
            ],
        )
        .map_err(map_sqlite)?;
    let mut value = current.value.expect("associated current filters exist");
    value.association = FilterAssociation::Custom;
    Ok(StoredCurrentFilters {
        state_revision: current.state_revision,
        revision: next,
        value: Some(value),
    })
}

struct RawSavedView {
    id: String,
    name: String,
    schema_version: i64,
    revision: i64,
    snapshot: String,
    created_at: i64,
    updated_at: i64,
}

fn raw_saved_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawSavedView> {
    Ok(RawSavedView {
        id: row.get(0)?,
        name: row.get(1)?,
        schema_version: row.get(2)?,
        revision: row.get(3)?,
        snapshot: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn parse_saved_view(raw: RawSavedView) -> PersonalStateResult<SavedViewDefinition> {
    let schema_version = nonnegative_i64_to_u64(raw.schema_version)?;
    let revision = nonnegative_i64_to_u64(raw.revision)?;
    if schema_version == 0 || revision == 0 {
        return Err(PersonalStateError::InvalidStoredValue {
            table: "personal_saved_views_v1",
            field: "revision",
        });
    }
    let snapshot = serde_json::from_str::<JsonValue>(&raw.snapshot)?;
    Ok(SavedViewDefinition {
        id: TraceId::from_str(&raw.id)?,
        name: raw.name,
        schema_version,
        revision: SavedViewRevision::from_stored(revision),
        content: decode_filter_snapshot(snapshot, schema_version),
        created_at: UnixMillis::try_from(raw.created_at)?,
        updated_at: UnixMillis::try_from(raw.updated_at)?,
    })
}

fn load_saved_view(
    connection: &Connection,
    id: TraceId,
) -> PersonalStateResult<Option<SavedViewDefinition>> {
    connection
        .query_row(
            "SELECT view_id, name, schema_version, revision, snapshot_json,
                    created_at_ms, updated_at_ms
             FROM personal_saved_views_v1 WHERE view_id = ?1",
            [id.to_string()],
            raw_saved_view,
        )
        .optional()?
        .map(parse_saved_view)
        .transpose()
}

fn require_saved_view(
    connection: &Connection,
    id: TraceId,
    expected: SavedViewRevision,
) -> PersonalStateResult<SavedViewDefinition> {
    let definition =
        load_saved_view(connection, id)?.ok_or(PersonalStateError::SavedViewNotFound { id })?;
    if definition.revision != expected {
        return Err(PersonalStateError::SavedViewRevisionConflict {
            id,
            expected: expected.get(),
            actual: definition.revision.get(),
        });
    }
    Ok(definition)
}

fn raw_snapshot(connection: &Connection, id: TraceId) -> PersonalStateResult<JsonValue> {
    let raw = connection.query_row(
        "SELECT snapshot_json FROM personal_saved_views_v1 WHERE view_id = ?1",
        [id.to_string()],
        |row| row.get::<_, String>(0),
    )?;
    serde_json::from_str(&raw).map_err(Into::into)
}

fn require_name_available(
    connection: &Connection,
    name_key: &str,
    except: Option<TraceId>,
) -> PersonalStateResult<()> {
    let existing = connection
        .query_row(
            "SELECT view_id, revision FROM personal_saved_views_v1
             WHERE name_key = ?1 AND (?2 IS NULL OR view_id <> ?2)",
            params![name_key, except.map(|id| id.to_string())],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?;
    if let Some((id, revision)) = existing {
        return Err(PersonalStateError::SavedViewNameConflict {
            existing_id: TraceId::from_str(&id)?,
            existing_revision: nonnegative_i64_to_u64(revision)?,
        });
    }
    Ok(())
}

fn normalize_saved_view_name(value: &str) -> PersonalStateResult<(String, String)> {
    let name = value.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err(PersonalStateError::InvalidSavedViewName);
    }
    let key = name
        .chars()
        .flat_map(char::to_lowercase)
        .collect::<String>();
    Ok((name.to_owned(), key))
}

fn next_saved_view_id(connection: &Connection) -> PersonalStateResult<TraceId> {
    let mut ids = SystemTraceIdSource;
    for _ in 0..16 {
        let id = ids.next_trace_id();
        let exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM personal_saved_views_v1 WHERE view_id = ?1)",
            [id.to_string()],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Ok(id);
        }
    }
    Err(PersonalStateError::SavedViewIdExhausted)
}

fn next_saved_view_revision(value: SavedViewRevision) -> PersonalStateResult<SavedViewRevision> {
    value
        .get()
        .checked_add(1)
        .ok_or(PersonalStateError::RevisionOverflow)
        .and_then(SavedViewRevision::try_from)
}

fn next_current_revision(
    value: CurrentFiltersRevision,
) -> PersonalStateResult<CurrentFiltersRevision> {
    value
        .get()
        .checked_add(1)
        .ok_or(PersonalStateError::RevisionOverflow)
        .and_then(CurrentFiltersRevision::try_from)
}

fn map_sqlite(error: SqliteError) -> PersonalStateError {
    match &error {
        SqliteError::SqliteFailure(detail, _)
            if detail.code == rusqlite::ffi::ErrorCode::DiskFull =>
        {
            PersonalStateError::StorageFull
        }
        _ => PersonalStateError::Sqlite(error),
    }
}

fn u64_to_i64(value: u64) -> PersonalStateResult<i64> {
    i64::try_from(value).map_err(|_| PersonalStateError::RevisionOverflow)
}

fn nonnegative_i64_to_u64(value: i64) -> PersonalStateResult<u64> {
    u64::try_from(value).map_err(|_| PersonalStateError::StoredIntegerOutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_full_is_a_distinct_product_error() {
        let error = SqliteError::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_FULL),
            Some("database or disk is full".to_owned()),
        );
        assert!(matches!(map_sqlite(error), PersonalStateError::StorageFull));
    }
}
