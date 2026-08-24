use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::path::Path;
use std::str::FromStr;

use bcsp_contracts::{
    FilterRequestV1, FilterValuesInputV1, NormalizedFilterValuesV1, OpenEpisodeId,
    OpenEpisodeState, SectionKey, TermId, TraceId, WatchContinuousDurationV1, WatchMaxAudible,
    WatchNotificationMode, WatchPolicyV1,
};
use bcsp_local_user_state::{
    CatalogRefreshMinutes, CurrentFiltersRevision, DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD,
    DESIRED_WATCH_TOMBSTONE_ROTATION_THRESHOLD, DesiredWatch, DesiredWatchBudgetKind,
    DesiredWatchCommand, DesiredWatchCommitted, DesiredWatchMutationOutcome,
    DesiredWatchReceiptOutcome, EpisodeActionInput, EpisodeActionKind, EpisodeDisposition,
    EpisodeHistoryIdentity, EpisodeSummaryInput, FilterAssociation, HistoryFilter,
    HistoryWriteOutcome, LocalSettings, LocaleOverride, MAX_DESIRED_WATCH_AUTHORITY_ROWS,
    MAX_DESIRED_WATCH_RECEIPTS, MAX_DESIRED_WATCH_TOMBSTONES, MAX_DESIRED_WATCHES,
    MAX_SELECTED_SECTIONS, OpenRefreshSeconds, PageRequest, PersonalStateError, PersonalStateStore,
    SavedViewContent, SavedViewIncompatibility, SavedViewMatch, SavedViewRevision,
    SelectionMutation, SettingsRevision, UnixMillis, UserStateRevision, VolumePercent,
    WatchFastLaneSeconds,
};
use rusqlite::Connection;
use serde_json::json;
use tempfile::TempDir;

fn database_path(directory: &TempDir) -> std::path::PathBuf {
    directory.path().join("rbcsp.sqlite")
}

fn trace(value: u64) -> TraceId {
    TraceId::from_str(&format!("00000000-0000-4000-8000-{value:012x}"))
        .expect("valid deterministic v4 trace ID")
}

fn section(value: u16) -> SectionKey {
    SectionKey::try_new("T2026F", "CAMPUS_A", &format!("{value:05}")).expect("valid SectionKey")
}

fn filters() -> FilterRequestV1 {
    filters_for("T2026F")
}

fn filters_for(term: &str) -> FilterRequestV1 {
    let input = FilterValuesInputV1::for_term(TermId::try_from(term).unwrap());
    FilterRequestV1::new(NormalizedFilterValuesV1::try_new(input).unwrap())
}

/// Writes one desired row straight to SQLite. The store exposes no writer:
/// the unfenced upsert was removed with the CAS design, and the fenced one
/// arrives with the authority actor.
fn seed_desired_watch(path: &std::path::Path, key: SectionKey, revision: i64) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute(
            "INSERT INTO personal_desired_watches_v1
                 (term_id, campus_code, section_index, desired, policy_json,
                  revision, materialization_epoch)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?5)",
            rusqlite::params![
                key.term().as_str(),
                key.campus().as_str(),
                key.index().as_str(),
                serde_json::to_string(&WatchPolicyV1::default()).unwrap(),
                revision,
            ],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE personal_state_metadata_v1
                SET desired_watch_revision_counter = MAX(desired_watch_revision_counter, ?1),
                    desired_watch_materialization_counter =
                        MAX(desired_watch_materialization_counter, ?1)",
            [revision],
        )
        .unwrap();
}

fn state_one() -> UserStateRevision {
    UserStateRevision::try_from(1).unwrap()
}

fn current_revision(value: u64) -> CurrentFiltersRevision {
    CurrentFiltersRevision::try_from(value).unwrap()
}

#[test]
fn consistent_read_keeps_one_wal_snapshot_across_a_concurrent_reset() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut writer = PersonalStateStore::open(&path).unwrap();
    writer
        .replace_current_filters(state_one(), CurrentFiltersRevision::ZERO, &filters())
        .unwrap();
    let reader = PersonalStateStore::open(&path).unwrap();
    let (first_read, first_read_rx) = std::sync::mpsc::sync_channel(0);
    let (writer_finished, writer_finished_rx) = std::sync::mpsc::sync_channel(0);

    let reader = std::thread::spawn(move || {
        reader.consistent_read(|store| {
            let state_revision = store.user_state_revision()?;
            first_read.send(()).unwrap();
            writer_finished_rx.recv().unwrap();
            let current_filters = store.current_filters()?;
            Ok((state_revision, current_filters))
        })
    });

    first_read_rx.recv().unwrap();
    let reset = writer.clear_personal_data(state_one()).unwrap();
    assert_eq!(reset.state_revision.get(), 2);
    writer_finished.send(()).unwrap();

    let (state_revision, current_filters) = reader.join().unwrap().unwrap();
    assert_eq!(state_revision, state_one());
    assert_eq!(current_filters.state_revision, state_one());
    assert_eq!(current_filters.revision, current_revision(1));
    assert!(current_filters.value.is_some());

    let after = PersonalStateStore::open(&path)
        .unwrap()
        .snapshot(PageRequest::DEFAULT)
        .unwrap();
    assert_eq!(after.state_revision.get(), 2);
    assert_eq!(after.current_filters.state_revision.get(), 2);
    assert_eq!(after.current_filters.revision, CurrentFiltersRevision::ZERO);
    assert!(after.current_filters.value.is_none());
}

fn identity(section: SectionKey, run: u64, episode: u64) -> EpisodeHistoryIdentity {
    EpisodeHistoryIdentity::new(section, trace(run), OpenEpisodeId::from(trace(episode)))
}

fn acknowledged_summary(identity: EpisodeHistoryIdentity) -> EpisodeSummaryInput {
    EpisodeSummaryInput {
        identity,
        state: OpenEpisodeState::Acknowledged,
        mode: WatchNotificationMode::OneShot,
        first_observed_at: UnixMillis::try_from(1_000).unwrap(),
        last_observed_at: UnixMillis::try_from(2_000).unwrap(),
        acknowledged_at: Some(UnixMillis::try_from(2_100).unwrap()),
        timed_out_at: None,
        closed_at: None,
        disposition: Some(EpisodeDisposition::Acknowledged),
        audible_count: 2,
        observation_count: NonZeroU64::new(3).unwrap(),
        last_observation_id: trace(900),
    }
}

fn insert_synthetic_operational_row(path: &Path) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE synthetic_operational(value TEXT NOT NULL) STRICT;
             INSERT INTO synthetic_operational(value) VALUES ('preserve-me');",
        )
        .unwrap();
}

#[test]
fn applied_filter_association_uses_the_camel_case_wire_contract() {
    let association = FilterAssociation::Applied {
        view_id: trace(1),
        revision: SavedViewRevision::try_from(2).unwrap(),
    };

    assert_eq!(
        serde_json::to_value(association).unwrap(),
        json!({
            "kind": "APPLIED",
            "viewId": "00000000-0000-4000-8000-000000000001",
            "revision": 2,
        })
    );
}

#[test]
fn incompatible_saved_view_uses_the_camel_case_wire_contract() {
    let content = SavedViewContent::Incompatible {
        raw_snapshot: json!({"future": true}),
        reason: SavedViewIncompatibility::UnknownField {
            stable_id: "FLT-FUTURE".to_owned(),
        },
    };

    assert_eq!(
        serde_json::to_value(content).unwrap(),
        json!({
            "status": "INCOMPATIBLE",
            "rawSnapshot": {"future": true},
            "reason": {
                "kind": "UNKNOWN_FIELD",
                "stableId": "FLT-FUTURE",
            },
        })
    );
}

#[test]
fn first_start_is_schema_only_and_enforces_sqlite_and_migration_contracts() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let store = PersonalStateStore::open(&path).unwrap();

    let configuration = store.sqlite_configuration().unwrap();
    assert_eq!(configuration.journal_mode, "wal");
    assert!(configuration.foreign_keys);
    assert!(configuration.busy_timeout_ms >= 5_000);
    assert_eq!(store.personal_table_counts().unwrap(), Default::default());
    let migrations = store.migration_records().unwrap();
    assert_eq!(migrations.len(), 4);
    assert_eq!(migrations[0].migration_id, 10_001);
    assert_eq!(migrations[1].migration_id, 10_002);
    assert_eq!(migrations[2].migration_id, 10_003);
    assert_eq!(migrations[3].migration_id, 10_004);
    assert_eq!(migrations[0].checksum.len(), 64);
    let _ = store.checkpoint_wal().unwrap();
    drop(store);

    let connection = Connection::open(&path).unwrap();
    let schema: String = connection
        .query_row(
            "SELECT group_concat(sql, ' ') FROM sqlite_schema
             WHERE type = 'table' AND name LIKE 'personal_%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!schema.to_ascii_lowercase().contains("connection"));
    assert!(!schema.to_ascii_lowercase().contains("active_watch"));
}

#[test]
fn migration_checksum_drift_fails_closed() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    drop(PersonalStateStore::open(&path).unwrap());
    Connection::open(&path)
        .unwrap()
        .execute(
            "UPDATE personal_migration_ledger SET checksum = ?1 WHERE migration_id = 10001",
            ["0".repeat(64)],
        )
        .unwrap();
    assert!(matches!(
        PersonalStateStore::open(&path),
        Err(PersonalStateError::MigrationChecksumMismatch {
            migration_id: 10_001
        })
    ));
}

#[test]
fn typed_settings_are_bounded_cas_persisted_and_saved_view_associated() {
    assert!(CatalogRefreshMinutes::try_from(1).is_ok());
    assert!(CatalogRefreshMinutes::try_from(1440).is_ok());
    assert!(CatalogRefreshMinutes::try_from(0).is_err());
    assert!(CatalogRefreshMinutes::try_from(1441).is_err());
    assert!(OpenRefreshSeconds::try_from(3).is_ok());
    assert!(OpenRefreshSeconds::try_from(3600).is_ok());
    assert!(OpenRefreshSeconds::try_from(2).is_err());
    assert!(OpenRefreshSeconds::try_from(3601).is_err());
    assert!(WatchFastLaneSeconds::try_from(3).is_ok());
    assert!(WatchFastLaneSeconds::try_from(60).is_ok());
    assert!(WatchFastLaneSeconds::try_from(2).is_err());
    assert!(WatchFastLaneSeconds::try_from(61).is_err());
    assert!(VolumePercent::try_from(100).is_ok());
    assert!(VolumePercent::try_from(101).is_err());

    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();
    assert_eq!(store.settings().unwrap().revision, SettingsRevision::ZERO);

    let settings = LocalSettings {
        locale_override: LocaleOverride::ZhCn,
        catalog_refresh_minutes: CatalogRefreshMinutes::try_from(1440).unwrap(),
        open_refresh_seconds: OpenRefreshSeconds::try_from(3600).unwrap(),
        watch_fast_lane_seconds: WatchFastLaneSeconds::try_from(60).unwrap(),
        volume_percent: VolumePercent::try_from(73).unwrap(),
        sound_policy: WatchPolicyV1::new(
            WatchNotificationMode::Continuous,
            WatchMaxAudible::try_from(9).unwrap(),
            WatchContinuousDurationV1::finite_seconds(600).unwrap(),
        ),
    };
    let stored = store
        .compare_and_swap_settings(
            UserStateRevision::try_from(1).unwrap(),
            SettingsRevision::ZERO,
            &settings,
        )
        .unwrap();
    assert_eq!(stored.revision.get(), 1);
    assert!(matches!(
        store.compare_and_swap_settings(
            UserStateRevision::try_from(1).unwrap(),
            SettingsRevision::ZERO,
            &settings
        ),
        Err(PersonalStateError::RevisionConflict {
            expected: 0,
            actual: 1
        })
    ));
    drop(store);
    assert_eq!(
        PersonalStateStore::open(&path).unwrap().settings().unwrap(),
        stored
    );
}

#[test]
fn legacy_settings_without_watch_fast_lane_use_the_ten_second_default() {
    let mut legacy = serde_json::to_value(LocalSettings::default()).expect("settings JSON");
    legacy
        .as_object_mut()
        .expect("settings object")
        .remove("watchFastLaneSeconds");
    let settings: LocalSettings = serde_json::from_value(legacy).expect("legacy settings");
    assert_eq!(settings.watch_fast_lane_seconds.get(), 10);
}

#[test]
fn saved_views_crud_is_cas_safe_and_tracks_clean_modified_association() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();
    let first_filters = filters();
    let changed_filters = filters_for("T2027S");
    for invalid in ["", "   ", "line\nbreak"] {
        assert!(matches!(
            store.create_saved_view(
                state_one(),
                CurrentFiltersRevision::ZERO,
                invalid,
                &first_filters,
                UnixMillis::try_from(999).unwrap(),
            ),
            Err(PersonalStateError::InvalidSavedViewName)
        ));
    }
    let created = store
        .create_saved_view(
            state_one(),
            CurrentFiltersRevision::ZERO,
            "  My View  ",
            &first_filters,
            UnixMillis::try_from(1_000).unwrap(),
        )
        .unwrap();
    assert_eq!(created.definition.name, "My View");
    assert_eq!(created.definition.revision.get(), 1);
    assert_eq!(created.current_filters.revision.get(), 1);
    assert_eq!(
        created.definition.match_current(&created.current_filters),
        SavedViewMatch::Clean
    );
    let id = created.definition.id;
    let (raw_revision, raw_snapshot) = store
        .saved_view_raw_snapshot(id)
        .unwrap()
        .expect("created Saved View has a raw snapshot");
    assert_eq!(raw_revision, created.definition.revision);
    assert_eq!(raw_snapshot["schemaVersion"], 3);
    assert_eq!(raw_snapshot["codecVersion"], 1);

    assert!(matches!(
        store.create_saved_view(
            state_one(),
            current_revision(1),
            "my view",
            &first_filters,
            UnixMillis::try_from(1_100).unwrap(),
        ),
        Err(PersonalStateError::SavedViewNameConflict {
            existing_revision: 1,
            ..
        })
    ));

    let edited = store
        .replace_current_filters(state_one(), current_revision(1), &changed_filters)
        .unwrap();
    assert!(matches!(
        edited.value.as_ref().unwrap().association,
        FilterAssociation::Applied { view_id, .. } if view_id == id
    ));
    assert_eq!(
        store
            .saved_view(id)
            .unwrap()
            .unwrap()
            .match_current(&edited),
        SavedViewMatch::Modified
    );
    let edited_back = store
        .replace_current_filters(state_one(), current_revision(2), &first_filters)
        .unwrap();
    assert_eq!(
        store
            .saved_view(id)
            .unwrap()
            .unwrap()
            .match_current(&edited_back),
        SavedViewMatch::Clean
    );

    let renamed = store
        .rename_saved_view(
            state_one(),
            current_revision(3),
            id,
            SavedViewRevision::try_from(1).unwrap(),
            "Renamed",
            UnixMillis::try_from(2_000).unwrap(),
        )
        .unwrap();
    assert_eq!(renamed.definition.revision.get(), 2);
    assert_eq!(renamed.current_filters.revision.get(), 4);
    assert!(matches!(
        renamed
            .current_filters
            .value
            .as_ref()
            .unwrap()
            .association,
        FilterAssociation::Applied { revision, .. } if revision.get() == 2
    ));
    assert!(matches!(
        store.rename_saved_view(
            state_one(),
            current_revision(4),
            id,
            SavedViewRevision::try_from(1).unwrap(),
            "stale",
            UnixMillis::try_from(2_100).unwrap(),
        ),
        Err(PersonalStateError::SavedViewRevisionConflict {
            expected: 1,
            actual: 2,
            ..
        })
    ));

    let updated = store
        .update_saved_view(
            state_one(),
            current_revision(4),
            id,
            SavedViewRevision::try_from(2).unwrap(),
            &changed_filters,
            UnixMillis::try_from(3_000).unwrap(),
        )
        .unwrap();
    assert_eq!(updated.definition.revision.get(), 3);
    assert_eq!(updated.current_filters.revision.get(), 5);
    assert_eq!(
        updated.definition.match_current(&updated.current_filters),
        SavedViewMatch::Clean
    );

    let duplicate = store
        .duplicate_saved_view(
            state_one(),
            id,
            SavedViewRevision::try_from(3).unwrap(),
            "Copy",
            UnixMillis::try_from(4_000).unwrap(),
        )
        .unwrap();
    assert_ne!(duplicate.definition.id, id);
    assert_eq!(duplicate.definition.revision.get(), 1);
    assert_eq!(duplicate.current_filters.revision.get(), 5);
    assert_eq!(store.saved_views().unwrap().len(), 2);
    let copy_id = duplicate.definition.id;
    drop(store);
    let mut store = PersonalStateStore::open(&path).unwrap();
    assert_eq!(store.saved_views().unwrap().len(), 2);
    assert_eq!(store.current_filters().unwrap().revision.get(), 5);
    let applied = store
        .apply_saved_view(
            state_one(),
            current_revision(5),
            copy_id,
            SavedViewRevision::try_from(1).unwrap(),
        )
        .unwrap();
    assert_eq!(applied.revision.get(), 6);
    let deleted = store
        .delete_saved_view(
            state_one(),
            current_revision(6),
            copy_id,
            SavedViewRevision::try_from(1).unwrap(),
        )
        .unwrap();
    assert!(matches!(
        deleted.current_filters.value.unwrap().association,
        FilterAssociation::Custom
    ));
    assert_eq!(deleted.current_filters.revision.get(), 7);
    let all = store
        .delete_all_saved_views(state_one(), current_revision(7))
        .unwrap();
    assert_eq!(all.deleted_views, 1);
    assert_eq!(all.current_filters.revision.get(), 7);
    assert!(store.saved_views().unwrap().is_empty());
}

#[test]
fn snapshot_codec_neutral_migrates_missing_fields_and_retains_unknown_raw() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();
    let created = store
        .create_saved_view(
            state_one(),
            CurrentFiltersRevision::ZERO,
            "Schema fixture",
            &filters(),
            UnixMillis::try_from(1_000).unwrap(),
        )
        .unwrap();
    let id = created.definition.id;
    drop(store);

    let connection = Connection::open(&path).unwrap();
    let raw: String = connection
        .query_row(
            "SELECT snapshot_json FROM personal_saved_views_v1 WHERE view_id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    let mut snapshot = serde_json::from_str::<serde_json::Value>(&raw).unwrap();
    snapshot["fields"]
        .as_object_mut()
        .unwrap()
        .remove("FLT-C02");
    connection
        .execute(
            "UPDATE personal_saved_views_v1 SET snapshot_json = ?1 WHERE view_id = ?2",
            [snapshot.to_string(), id.to_string()],
        )
        .unwrap();
    drop(connection);
    let compatible = PersonalStateStore::open(&path)
        .unwrap()
        .saved_view(id)
        .unwrap()
        .unwrap();
    assert!(matches!(
        compatible.content,
        SavedViewContent::Compatible { .. }
    ));

    let connection = Connection::open(&path).unwrap();
    snapshot["fields"]["FLT-REMOVED"] = json!({"future": true});
    connection
        .execute(
            "UPDATE personal_saved_views_v1 SET snapshot_json = ?1 WHERE view_id = ?2",
            [snapshot.to_string(), id.to_string()],
        )
        .unwrap();
    drop(connection);
    let mut store = PersonalStateStore::open(&path).unwrap();
    let incompatible = store.saved_view(id).unwrap().unwrap();
    assert!(matches!(
        &incompatible.content,
        SavedViewContent::Incompatible {
            raw_snapshot,
            reason: SavedViewIncompatibility::UnknownField { stable_id },
        } if raw_snapshot["fields"]["FLT-REMOVED"] == json!({"future": true})
            && stable_id == "FLT-REMOVED"
    ));
    assert_eq!(
        incompatible.match_current(&store.current_filters().unwrap()),
        SavedViewMatch::Incompatible
    );
    assert!(matches!(
        store.apply_saved_view(
            state_one(),
            current_revision(1),
            id,
            SavedViewRevision::try_from(1).unwrap(),
        ),
        Err(PersonalStateError::SavedViewIncompatible {
            reason: SavedViewIncompatibility::UnknownField { .. },
            ..
        })
    ));
    drop(store);
    let raw_after: String = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT snapshot_json FROM personal_saved_views_v1 WHERE view_id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&raw_after).unwrap(),
        snapshot
    );
}

#[test]
fn saved_view_library_has_no_product_count_cap_or_eviction() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();
    for index in 0..128_u64 {
        store
            .create_saved_view(
                state_one(),
                current_revision(index),
                &format!("View {index:03}"),
                &filters(),
                UnixMillis::try_from(1_000 + index as i64).unwrap(),
            )
            .unwrap();
    }
    let views = store.saved_views().unwrap();
    assert_eq!(views.len(), 128);
    assert_eq!(views.first().unwrap().name, "View 000");
    assert_eq!(views.last().unwrap().name, "View 127");
}

#[test]
fn legacy_settings_current_filters_migrate_to_the_dedicated_snapshot_table() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    drop(PersonalStateStore::open(&path).unwrap());
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "DELETE FROM personal_migration_ledger
                 WHERE migration_id IN (10002, 10003, 10004);
             DROP TABLE personal_desired_watch_receipts_v1;
             DROP TABLE personal_desired_watches_v1;
             DROP TABLE personal_saved_views_v1;
             DROP TABLE personal_current_filters_v1;
             DROP TABLE personal_state_metadata_v1;",
        )
        .unwrap();
    let mut legacy_settings = serde_json::to_value(LocalSettings::default()).unwrap();
    legacy_settings["currentFilters"] = json!({
        "association": {"kind": "CUSTOM"},
        "filters": filters(),
    });
    connection
        .execute(
            "INSERT INTO personal_settings_v1(singleton_id, revision, settings_json)
             VALUES (1, 1, ?1)",
            [legacy_settings.to_string()],
        )
        .unwrap();
    drop(connection);

    let store = PersonalStateStore::open(&path).unwrap();
    assert_eq!(store.migration_records().unwrap().len(), 4);
    assert_eq!(store.settings().unwrap().value, LocalSettings::default());
    let migrated = store.current_filters().unwrap();
    assert_eq!(migrated.revision.get(), 1);
    assert_eq!(migrated.value.unwrap().filters(), Some(&filters()));
}

#[test]
fn selection_persists_order_and_the_tenth_section_is_explicitly_rejected() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();
    let state_revision = store.user_state_revision().unwrap();
    for value in 1..=MAX_SELECTED_SECTIONS as u16 {
        assert_eq!(
            store
                .add_selected_section(state_revision, section(value))
                .unwrap(),
            SelectionMutation::Added {
                position: (value - 1) as u8
            }
        );
    }
    assert!(matches!(
        store.add_selected_section(state_revision, section(10)),
        Err(PersonalStateError::SelectionLimitExceeded { maximum: 9 })
    ));
    assert!(matches!(
        store
            .add_selected_section(state_revision, section(1))
            .unwrap(),
        SelectionMutation::AlreadySelected { position: 0 }
    ));
    let expected = (1..=9).map(section).collect::<Vec<_>>();
    drop(store);
    assert_eq!(
        PersonalStateStore::open(&path)
            .unwrap()
            .selected_sections()
            .unwrap(),
        expected
    );
}

#[test]
fn episode_summaries_and_actions_support_aggregation_filter_paging_and_delete() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();
    let first = acknowledged_summary(identity(section(1), 100, 200));
    let mut second = acknowledged_summary(identity(section(2), 101, 201));
    second.state = OpenEpisodeState::TimedOut;
    second.acknowledged_at = None;
    second.timed_out_at = Some(UnixMillis::try_from(2_200).unwrap());
    second.disposition = Some(EpisodeDisposition::TimedOut);
    store.upsert_episode_summary(&first).unwrap();
    store.upsert_episode_summary(&second).unwrap();

    let action = EpisodeActionInput {
        action_id: trace(300),
        identity: first.identity.clone(),
        kind: EpisodeActionKind::CuePlayed,
        occurred_at: UnixMillis::try_from(2_200).unwrap(),
    };
    assert_eq!(
        store.append_episode_action(&action).unwrap(),
        HistoryWriteOutcome::Inserted
    );
    assert_eq!(
        store.append_episode_action(&action).unwrap(),
        HistoryWriteOutcome::AlreadyPresent
    );
    store
        .append_episode_action(&EpisodeActionInput {
            action_id: trace(301),
            identity: first.identity.clone(),
            kind: EpisodeActionKind::AlertDismissed,
            occurred_at: UnixMillis::try_from(2_300).unwrap(),
        })
        .unwrap();

    let mut closed = first.clone();
    closed.state = OpenEpisodeState::Closed;
    closed.last_observed_at = UnixMillis::try_from(2_500).unwrap();
    closed.closed_at = Some(UnixMillis::try_from(3_000).unwrap());
    closed.disposition = Some(EpisodeDisposition::SectionClosed);
    store.upsert_episode_summary(&closed).unwrap();

    let page = store
        .episode_history(
            &HistoryFilter {
                section_key: Some(section(1)),
                ..Default::default()
            },
            PageRequest::try_new(0, 1).unwrap(),
        )
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].action_count, 2);
    assert_eq!(page.items[0].mode, WatchNotificationMode::OneShot);
    assert_eq!(page.items[0].acknowledged_at.unwrap().get(), 2_100);
    assert_eq!(page.items[0].closed_at.unwrap().get(), 3_000);
    assert_eq!(
        page.items[0].disposition,
        Some(EpisodeDisposition::SectionClosed)
    );
    assert_eq!(page.items[0].last_observation_id, trace(900));

    let actions = store
        .episode_actions(
            &HistoryFilter {
                action_kind: Some(EpisodeActionKind::CuePlayed),
                ..Default::default()
            },
            PageRequest::DEFAULT,
        )
        .unwrap();
    assert_eq!(actions.total, 1);
    assert!(store.delete_episode_action(trace(301)).unwrap());
    assert!(store.delete_episode_history(&first.identity).unwrap());
    assert_eq!(
        store
            .episode_actions(&HistoryFilter::default(), PageRequest::DEFAULT)
            .unwrap()
            .total,
        0
    );
}

#[test]
fn restart_snapshot_never_restores_active_watch_and_reset_preserves_operational_rows() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    insert_synthetic_operational_row(&path);
    let mut store = PersonalStateStore::open(&path).unwrap();
    let settings = LocalSettings {
        locale_override: LocaleOverride::EnUs,
        ..Default::default()
    };
    store
        .compare_and_swap_settings(
            store.user_state_revision().unwrap(),
            SettingsRevision::ZERO,
            &settings,
        )
        .unwrap();
    store
        .replace_selected_sections(
            store.user_state_revision().unwrap(),
            &[section(1), section(2)],
        )
        .unwrap();
    // Desired-watch INTENT persists across restarts (L1); the live watch
    // count never does. Seeded directly: the CAS writer lands with the
    // authority actor, and no unfenced writer exists for a caller to reach.
    seed_desired_watch(&path, section(1), 1);
    seed_desired_watch(&path, section(2), 2);
    let saved = store
        .create_saved_view(
            state_one(),
            CurrentFiltersRevision::ZERO,
            "Preserved until user reset",
            &filters(),
            UnixMillis::try_from(900).unwrap(),
        )
        .unwrap();
    let filter_reset = store
        .reset_current_filters(
            state_one(),
            current_revision(1),
            Some(&filters_for("T2027S")),
        )
        .unwrap();
    assert_eq!(store.saved_views().unwrap().len(), 1);
    assert!(matches!(
        filter_reset.value.unwrap().association,
        FilterAssociation::Custom
    ));
    let library_reset = store
        .delete_all_saved_views(state_one(), current_revision(2))
        .unwrap();
    assert_eq!(library_reset.deleted_views, 1);
    assert!(store.saved_views().unwrap().is_empty());
    assert_eq!(
        library_reset.current_filters.value.unwrap().filters(),
        Some(&filters_for("T2027S"))
    );
    let recreated = store
        .create_saved_view(
            state_one(),
            current_revision(2),
            "Reset with all local data",
            &filters(),
            UnixMillis::try_from(950).unwrap(),
        )
        .unwrap();
    assert_ne!(recreated.definition.id, saved.definition.id);
    let summary = acknowledged_summary(identity(section(1), 110, 210));
    store.upsert_episode_summary(&summary).unwrap();
    store
        .append_episode_action(&EpisodeActionInput {
            action_id: trace(310),
            identity: summary.identity,
            kind: EpisodeActionKind::Opened,
            occurred_at: UnixMillis::try_from(1_100).unwrap(),
        })
        .unwrap();
    drop(store);

    let mut store = PersonalStateStore::open(&path).unwrap();
    let snapshot = store.snapshot(PageRequest::DEFAULT).unwrap();
    assert_eq!(snapshot.active_watch_count, 0);
    assert_eq!(
        snapshot.settings.value.locale_override,
        LocaleOverride::EnUs
    );
    assert_eq!(snapshot.selected_sections, vec![section(1), section(2)]);
    assert_eq!(
        snapshot
            .desired_watches
            .iter()
            .map(|watch| watch.section.clone())
            .collect::<Vec<_>>(),
        vec![section(1), section(2)],
        "desired-watch intent survives the restart",
    );
    assert_eq!(snapshot.episode_history.total, 1);
    let reset = store
        .clear_personal_data(store.user_state_revision().unwrap())
        .unwrap();
    assert_eq!(reset.deleted_settings, 1);
    assert_eq!(reset.deleted_current_filters, 1);
    assert_eq!(reset.deleted_saved_views, 1);
    assert_eq!(reset.deleted_selected_sections, 2);
    assert_eq!(reset.deleted_desired_watches, 2);
    assert_eq!(reset.deleted_episode_summaries, 1);
    assert_eq!(reset.deleted_episode_actions, 1);
    assert_eq!(store.personal_table_counts().unwrap(), Default::default());
    assert_eq!(reset.state_revision.get(), 2);
    assert!(matches!(
        store.compare_and_swap_settings(
            state_one(),
            SettingsRevision::ZERO,
            &LocalSettings::default()
        ),
        Err(PersonalStateError::UserStateRevisionConflict {
            expected: 1,
            actual: 2
        })
    ));
    assert!(matches!(
        store.add_selected_section(state_one(), section(3)),
        Err(PersonalStateError::UserStateRevisionConflict {
            expected: 1,
            actual: 2
        })
    ));
    assert_eq!(
        store
            .snapshot(PageRequest::DEFAULT)
            .unwrap()
            .active_watch_count,
        0
    );
    drop(store);

    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT value FROM synthetic_operational", [], |row| row
                .get::<_, String>(
                0
            ))
            .unwrap(),
        "preserve-me"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM personal_migration_ledger",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        4
    );
    let serialized = serde_json::to_value(snapshot).unwrap();
    assert_eq!(serialized["activeWatchCount"], json!(0));
    assert_eq!(serialized["desiredWatches"].as_array().unwrap().len(), 2);
}

#[test]
fn migration_10004_upgrades_intent_rows_and_separates_authority_from_the_wire() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    drop(PersonalStateStore::open(&path).unwrap());

    // Rewind to the 10003 shape and re-seed two intent rows, so reopening
    // exercises the real upgrade rather than a freshly built table.
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "DELETE FROM personal_migration_ledger WHERE migration_id = 10004;
             DROP TABLE personal_desired_watch_receipts_v1;
             DROP TABLE personal_desired_watches_v1;
             CREATE TABLE personal_desired_watches_v1 (
                 term_id TEXT NOT NULL,
                 campus_code TEXT NOT NULL,
                 section_index TEXT NOT NULL,
                 policy_json TEXT NOT NULL CHECK (json_valid(policy_json)),
                 PRIMARY KEY (term_id, campus_code, section_index)
             ) STRICT;
             ALTER TABLE personal_state_metadata_v1
                 RENAME TO personal_state_metadata_post10004;
             CREATE TABLE personal_state_metadata_v1 (
                 singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
                 state_revision INTEGER NOT NULL CHECK (state_revision > 0)
             ) STRICT;
             INSERT INTO personal_state_metadata_v1(singleton_id, state_revision)
                 SELECT singleton_id, state_revision FROM personal_state_metadata_post10004;
             DROP TABLE personal_state_metadata_post10004;",
        )
        .unwrap();
    let policy_json = serde_json::to_string(&WatchPolicyV1::default()).unwrap();
    for index in [2u16, 1u16] {
        let key = section(index);
        connection
            .execute(
                "INSERT INTO personal_desired_watches_v1
                     (term_id, campus_code, section_index, policy_json)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    key.term().as_str(),
                    key.campus().as_str(),
                    key.index().as_str(),
                    policy_json,
                ],
            )
            .unwrap();
    }
    drop(connection);

    let store = PersonalStateStore::open(&path).unwrap();
    assert_eq!(store.migration_records().unwrap().len(), 4);

    let authority = store.desired_watch_authority().unwrap();
    assert_eq!(authority.counters.authority_generation, 1);
    assert_eq!(authority.counters.actor_incarnation, 1);
    // Revisions are assigned in section-key order regardless of insert order,
    // and the counters are repaired to the highest value handed out.
    assert_eq!(
        authority
            .entries
            .iter()
            .map(|entry| (entry.section.clone(), entry.revision, entry.materialization_epoch))
            .collect::<Vec<_>>(),
        vec![(section(1), 1, 1), (section(2), 2, 2)],
    );
    assert_eq!(authority.counters.revision_counter, 2);
    assert_eq!(authority.counters.materialization_counter, 2);
    assert!(authority.entries.iter().all(|entry| !entry.is_tombstone()));

    // A tombstone is authority state, not wire state: it must reach the
    // authority reader (a late command needs its revision to compare against)
    // and must NOT reach the bootstrap, whose shape stays protocol-v1 exact.
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE personal_desired_watches_v1
                SET desired = 0, policy_json = NULL, revision = 3
              WHERE section_index = ?1",
            [section(1).index().as_str()],
        )
        .unwrap();
    drop(connection);

    let store = PersonalStateStore::open(&path).unwrap();
    let authority = store.desired_watch_authority().unwrap();
    assert_eq!(authority.entries.len(), 2, "the tombstone survives");
    let tombstone = authority
        .entries
        .iter()
        .find(|entry| entry.section == section(1))
        .expect("tombstone row");
    assert!(tombstone.is_tombstone());
    assert_eq!(tombstone.revision, 3);

    assert_eq!(
        store.desired_watches().unwrap(),
        vec![DesiredWatch {
            section: section(2),
            policy: WatchPolicyV1::default(),
        }],
        "the bootstrap view hides tombstones",
    );
    // The table count is authority-shaped (tombstones included) on purpose:
    // Full Reset has to delete them too.
    assert_eq!(store.personal_table_counts().unwrap().desired_watches, 2);
}

#[test]
fn the_desired_watch_table_rejects_a_row_that_disagrees_with_itself() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let store = PersonalStateStore::open(&path).unwrap();
    drop(store);
    let connection = Connection::open(&path).unwrap();
    let key = section(1);
    // desired = 1 without a policy, and desired = 0 with one: the CHECK ties
    // the two together so no reader has to defend against a half-row.
    for (desired, policy) in [(1, None), (0, Some("{}".to_owned()))] {
        let error = connection.execute(
            "INSERT INTO personal_desired_watches_v1
                 (term_id, campus_code, section_index, desired, policy_json,
                  revision, materialization_epoch)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, 1)",
            rusqlite::params![
                key.term().as_str(),
                key.campus().as_str(),
                key.index().as_str(),
                desired,
                policy,
            ],
        );
        assert!(error.is_err(), "desired = {desired} must not stand alone");
    }
}

#[test]
fn full_reset_is_the_authority_generation_barrier() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();
    seed_desired_watch(&path, section(1), 1);
    seed_desired_watch(&path, section(2), 2);

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "INSERT INTO personal_desired_watch_receipts_v1
                 (authority_generation, mutation_id, term_id, campus_code,
                  section_index, fingerprint, outcome_json)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, '{}')",
            rusqlite::params![
                "00000000-0000-4000-8000-000000000001",
                section(1).term().as_str(),
                section(1).campus().as_str(),
                section(1).index().as_str(),
                "a".repeat(64),
            ],
        )
        .unwrap();
    drop(connection);

    let before = store.desired_watch_authority().unwrap();
    assert_eq!(before.counters.authority_generation, 1);
    assert_eq!(before.counters.revision_counter, 2);
    assert_eq!(store.personal_table_counts().unwrap().desired_watch_receipts, 1);

    let reset = store.clear_personal_data(state_one()).unwrap();
    assert_eq!(reset.deleted_desired_watches, 2);
    assert_eq!(reset.deleted_desired_watch_receipts, 1);

    // A reset that only deleted rows would leave every pre-reset command
    // still carrying a matching generation, so it would be admitted right
    // after the wipe. The bump is what makes those commands stale.
    let after = store.desired_watch_authority().unwrap();
    assert_eq!(after.counters.authority_generation, 2);
    assert_eq!(after.counters.revision_counter, 0);
    assert_eq!(after.counters.materialization_counter, 0);
    assert_eq!(
        after.counters.actor_incarnation, 1,
        "the incarnation orders frames, not data, and must survive a reset",
    );
    assert!(after.entries.is_empty());
    assert_eq!(store.personal_table_counts().unwrap(), Default::default());
}

#[test]
fn table_counts_compose_inside_a_callers_consistent_read() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let store = PersonalStateStore::open(&path).unwrap();
    seed_desired_watch(&path, section(1), 1);

    // personal_table_counts() must not open a transaction of its own: a caller
    // that already holds a snapshot would otherwise get "cannot start a
    // transaction within a transaction".
    let (outer, inner) = store
        .consistent_read(|store| {
            let counts = store.personal_table_counts()?;
            Ok((store.desired_watches()?.len(), counts.desired_watches))
        })
        .expect("counts compose inside consistent_read");
    assert_eq!(outer, 1);
    assert_eq!(inner, 1);
}

/// A user START for `section`, based on the revision the caller claims to
/// have read.
fn start(section: SectionKey, based_on_revision: u64, mutation: u64) -> DesiredWatchCommand {
    DesiredWatchCommand {
        section,
        policy: Some(WatchPolicyV1::default()),
        based_on_revision,
        authority_generation: 1,
        mutation_id: trace(mutation),
    }
}

/// A user STOP, which leaves a tombstone rather than deleting the row.
fn stop(section: SectionKey, based_on_revision: u64, mutation: u64) -> DesiredWatchCommand {
    DesiredWatchCommand {
        section,
        policy: None,
        based_on_revision,
        authority_generation: 1,
        mutation_id: trace(mutation),
    }
}

/// A policy distinguishable from the default, for policy-only edits.
fn loud_policy() -> WatchPolicyV1 {
    WatchPolicyV1::new(
        WatchNotificationMode::Continuous,
        WatchMaxAudible::try_from(7).unwrap(),
        WatchContinuousDurationV1::finite_seconds(300).unwrap(),
    )
}

fn committed(outcome: DesiredWatchMutationOutcome) -> DesiredWatchCommitted {
    match outcome {
        DesiredWatchMutationOutcome::Committed(commit) => commit,
        other => panic!("expected a commit, got {other:?}"),
    }
}

fn receipt_count(store: &PersonalStateStore) -> u64 {
    store
        .personal_table_counts()
        .unwrap()
        .desired_watch_receipts
}

/// The ledger row for one mutation id, as it is actually stored.
fn stored_receipt(path: &Path, mutation: u64) -> (String, String) {
    let connection = Connection::open(path).unwrap();
    connection
        .query_row(
            "SELECT fingerprint, outcome_json
             FROM personal_desired_watch_receipts_v1
             WHERE authority_generation = 1 AND mutation_id = ?1",
            [trace(mutation).to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap()
}

#[test]
fn the_cas_refusals_are_ordered_so_one_answer_never_hides_another() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    // A command from a generation that no longer exists is refused before
    // anything else looks at it -- before the id, the revision, or the cap.
    // Reporting a stale revision here would tell a caller to retry against a
    // number from an authority that was thrown away.
    let mut from_the_past = start(section(1), 999, 1);
    from_the_past.authority_generation = 7;
    assert_eq!(
        store
            .commit_desired_watch_mutation(&from_the_past)
            .unwrap(),
        DesiredWatchMutationOutcome::StaleGeneration { current: 1 },
    );
    assert_eq!(
        receipt_count(&store),
        0,
        "a stale generation is decided before the ledger is consulted at all",
    );

    // A stale revision is decided before the cap: the caller has to re-read
    // either way, and a cap message would send it looking for the wrong fix.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(1), 4, 2))
            .unwrap(),
        DesiredWatchMutationOutcome::StaleRevision { current: 0 },
    );

    // The product cap is checked last, so a command that fails the revision
    // check is never told it is at the cap instead. Nine sections are
    // desired by the time the last assertion runs, so the cap WOULD fire if
    // the two checks were reordered.
    for index in 1..=MAX_DESIRED_WATCHES as u16 {
        committed(
            store
                .commit_desired_watch_mutation(&start(section(index), 0, 100 + u64::from(index)))
                .unwrap(),
        );
    }
    let mut past_the_cap = start(section(50), 4, 3);
    past_the_cap.policy = Some(loud_policy());
    assert_eq!(
        store.commit_desired_watch_mutation(&past_the_cap).unwrap(),
        DesiredWatchMutationOutcome::StaleRevision { current: 0 },
        "a stale revision outranks the cap: the caller has to re-read either way, \
         and a cap message would send it looking for the wrong fix",
    );
    assert_eq!(store.desired_watches().unwrap().len(), MAX_DESIRED_WATCHES);
}

#[test]
fn a_terminal_rejection_is_recorded_so_the_world_cannot_change_the_answer() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    // One tab submits against a row it believes exists at revision 1. There
    // is no row, so this is refused, terminally.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(1), 1, 1))
            .unwrap(),
        DesiredWatchMutationOutcome::StaleRevision { current: 0 },
    );
    assert_eq!(
        receipt_count(&store),
        1,
        "a terminal rejection is an answer, and answers are recorded",
    );
    assert_eq!(
        store.desired_watches().unwrap(),
        Vec::new(),
        "recording the refusal must not write the row it refused",
    );
    let counters = store.desired_watch_authority().unwrap().counters;
    assert_eq!(counters.revision_counter, 0, "nor advance a counter");
    assert_eq!(counters.materialization_counter, 0);

    // Another tab now creates that row, so revision 1 exists after all.
    committed(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 2))
            .unwrap(),
    );

    // The first tab retries the identical mutation. Without the recorded
    // refusal it would now pass the revision check and commit -- the same
    // mutation the user was told was rejected would quietly succeed, on a
    // row someone else created.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(1), 1, 1))
            .unwrap(),
        DesiredWatchMutationOutcome::Replayed(DesiredWatchReceiptOutcome::StaleRevision {
            current: 0
        }),
    );
    assert_eq!(store.desired_watches().unwrap().len(), 1);
}

#[test]
fn a_freed_slot_cannot_turn_a_recorded_limit_rejection_into_a_commit() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let mut revisions = Vec::new();
    for index in 1..=MAX_DESIRED_WATCHES as u16 {
        revisions.push(
            committed(
                store
                    .commit_desired_watch_mutation(&start(section(index), 0, u64::from(index)))
                    .unwrap(),
            )
            .revision,
        );
    }
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(10), 0, 10))
            .unwrap(),
        DesiredWatchMutationOutcome::LimitExceeded { maximum: 9 },
    );

    // Free a slot, then present the refused mutation again unchanged.
    committed(
        store
            .commit_desired_watch_mutation(&stop(section(1), revisions[0], 11))
            .unwrap(),
    );
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(10), 0, 10))
            .unwrap(),
        DesiredWatchMutationOutcome::Replayed(DesiredWatchReceiptOutcome::LimitExceeded {
            maximum: 9
        }),
        "the refusal stands; only a new gesture with a new id may ask again",
    );
    assert_eq!(store.desired_watches().unwrap().len(), 8);

    // A new gesture, with a new id, is admitted into the slot that opened.
    committed(
        store
            .commit_desired_watch_mutation(&start(section(10), 0, 12))
            .unwrap(),
    );
    assert_eq!(store.desired_watches().unwrap().len(), 9);
}

/// The authority is blind to the catalog, the term window and the campus
/// list, and that is the contract rather than an omission.
///
/// Every one of those conditions is revocable: a campus can leave the product
/// target list, a term can fall out of the window, a section can vanish from a
/// rebuild, the integrity gate can hold a snapshot. None of them is the user's
/// intent changing. Deciding them here would either freeze a passing condition
/// into a receipted permanent refusal -- the user is told "no", the section is
/// published a minute later, and the same id must still be answered "no"
/// forever -- or throw the intent away because a snapshot was rebuilding.
/// They are checked when the intent is MATERIALIZED, where the answer is
/// allowed to change back.
#[test]
fn the_writer_never_asks_whether_a_section_can_actually_be_watched() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    // Not a product campus, not a term in any window, and an index no
    // catalog in this database has ever published -- this store holds no
    // catalog at all. It commits, because the row records what the user
    // asked for, not what the world currently allows.
    let nowhere = SectionKey::try_new("90001", "ZZ", "99999").expect("valid SectionKey");
    let written = committed(
        store
            .commit_desired_watch_mutation(&DesiredWatchCommand {
                section: nowhere.clone(),
                policy: Some(WatchPolicyV1::default()),
                based_on_revision: 0,
                authority_generation: 1,
                mutation_id: trace(1),
            })
            .unwrap(),
    );
    assert_eq!(
        store.desired_watches().unwrap(),
        vec![DesiredWatch {
            section: nowhere.clone(),
            policy: WatchPolicyV1::default(),
        }],
    );
    assert_eq!(
        stored_receipt(&path, 1).1,
        format!(
            r#"{{"outcome":"COMMITTED","revision":{},"materializationEpoch":{},"epochChanged":true}}"#,
            written.revision, written.materialization_epoch,
        ),
        "the ledger records the commit, and no verdict about the section",
    );

    // And the row stays. Nothing in this crate can withdraw it: the only
    // writers are a user command, a rotation that renumbers it, and the Full
    // Reset that clears the whole table.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&stop(nowhere, written.revision, 2))
            .unwrap(),
        DesiredWatchMutationOutcome::Committed(DesiredWatchCommitted {
            revision: written.revision + 1,
            materialization_epoch: written.materialization_epoch + 1,
            epoch_changed: true,
        }),
        "only the user's own STOP clears it",
    );
}

/// Two pages that read the same revision race one another. Exactly one wins,
/// and the loser's stale command cannot resurrect what the winner cancelled.
#[test]
fn two_pages_holding_the_same_revision_cannot_both_commit() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let armed = committed(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
    );

    // Both pages read `armed.revision`. One of them stops the section.
    let stopped = committed(
        store
            .commit_desired_watch_mutation(&stop(section(1), armed.revision, 2))
            .unwrap(),
    );

    // The other page, still holding the pre-stop revision, submits a policy
    // edit. Without the tombstone this would find no row, read 0, and be
    // admitted -- re-arming a section the user just cancelled on another tab.
    let mut edit = start(section(1), armed.revision, 3);
    edit.policy = Some(loud_policy());
    assert_eq!(
        store.commit_desired_watch_mutation(&edit).unwrap(),
        DesiredWatchMutationOutcome::StaleRevision {
            current: stopped.revision
        },
    );
    assert_eq!(store.desired_watches().unwrap(), Vec::new());

    // The same id stays refused forever; only re-reading and resubmitting as
    // a NEW gesture gets the user their way.
    assert_eq!(
        store.commit_desired_watch_mutation(&edit).unwrap(),
        DesiredWatchMutationOutcome::Replayed(DesiredWatchReceiptOutcome::StaleRevision {
            current: stopped.revision
        }),
    );
    let mut fresh = start(section(1), stopped.revision, 4);
    fresh.policy = Some(loud_policy());
    committed(store.commit_desired_watch_mutation(&fresh).unwrap());
    assert_eq!(store.desired_watches().unwrap().len(), 1);
}

#[test]
fn a_repeated_mutation_id_replays_and_a_reused_one_reports_the_row_it_finds() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let first = committed(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
    );
    assert_eq!(receipt_count(&store), 1);

    // The same command again replays its original outcome. Evaluating it a
    // second time would burn a revision and hand the caller a number its
    // first attempt never saw.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
        DesiredWatchMutationOutcome::Replayed(DesiredWatchReceiptOutcome::committed(first)),
    );
    let authority = store.desired_watch_authority().unwrap();
    assert_eq!(authority.entries.len(), 1);
    assert_eq!(authority.entries[0].revision, first.revision);
    assert_eq!(
        authority.entries[0].materialization_epoch,
        first.materialization_epoch
    );
    assert_eq!(authority.counters.revision_counter, first.revision);
    assert_eq!(
        receipt_count(&store),
        1,
        "a replay inserts no second receipt"
    );

    // The same id carrying a different command is a collision, reported from
    // the row already there. A second insert would destroy the evidence.
    let outcome = store
        .commit_desired_watch_mutation(&start(section(2), 0, 1))
        .unwrap();
    let DesiredWatchMutationOutcome::MutationIdConflict(recorded) = outcome else {
        panic!("expected a conflict, got {outcome:?}");
    };
    assert_eq!(
        recorded.section,
        section(1),
        "the conflict names the id's original section"
    );
    assert_eq!(
        recorded.outcome,
        DesiredWatchReceiptOutcome::committed(first)
    );
    assert_eq!(receipt_count(&store), 1);
    assert_eq!(
        store.desired_watches().unwrap().len(),
        1,
        "the colliding command must not have written a row",
    );

    // The likelier collision is the same id under the same section carrying a
    // different command, which is why the fingerprint is compared and not
    // just the section.
    let mut same_section = start(section(1), 0, 1);
    same_section.policy = Some(loud_policy());
    let outcome = store
        .commit_desired_watch_mutation(&same_section)
        .unwrap();
    assert!(
        matches!(outcome, DesiredWatchMutationOutcome::MutationIdConflict(_)),
        "a different command under the same id and section still collides: {outcome:?}",
    );
    assert_eq!(receipt_count(&store), 1);
}

#[test]
fn a_tombstone_keeps_a_late_start_from_resurrecting_cancelled_intent() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let armed = committed(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
    );
    let stopped = committed(
        store
            .commit_desired_watch_mutation(&stop(section(1), armed.revision, 2))
            .unwrap(),
    );
    assert!(stopped.revision > armed.revision);
    assert!(stopped.epoch_changed, "stopping changes the desired value");
    assert!(stopped.materialization_epoch > armed.materialization_epoch);

    // The row is still there as a tombstone, and the bootstrap view hides it.
    let authority = store.desired_watch_authority().unwrap();
    assert_eq!(authority.entries.len(), 1);
    assert!(authority.entries[0].is_tombstone());
    assert_eq!(store.desired_watches().unwrap(), Vec::new());

    // A START held up in flight still carries the revision it read before the
    // stop. Without the tombstone it would find nothing, read that as 0, and
    // be admitted -- re-arming a section the user just cancelled.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(1), armed.revision, 3))
            .unwrap(),
        DesiredWatchMutationOutcome::StaleRevision {
            current: stopped.revision
        },
    );
    assert_eq!(store.desired_watches().unwrap(), Vec::new());
}

#[test]
fn the_cap_counts_the_state_a_mutation_leaves_behind() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let mut revisions = Vec::new();
    for index in 1..=MAX_DESIRED_WATCHES as u16 {
        revisions.push(
            committed(
                store
                    .commit_desired_watch_mutation(&start(section(index), 0, u64::from(index)))
                    .unwrap(),
            )
            .revision,
        );
    }
    assert_eq!(store.desired_watches().unwrap().len(), MAX_DESIRED_WATCHES);

    // A tenth section is refused, and refused before admission.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(10), 0, 10))
            .unwrap(),
        DesiredWatchMutationOutcome::LimitExceeded { maximum: 9 },
    );

    // Editing the policy of one of the nine still commits: the post-state is
    // nine either way. Testing the state a mutation FOUND would refuse this
    // and make a full watch list uneditable.
    let mut edit = start(section(1), revisions[0], 100);
    edit.policy = Some(loud_policy());
    let edited = committed(store.commit_desired_watch_mutation(&edit).unwrap());
    assert!(edited.revision > revisions[0]);
    assert!(
        !edited.epoch_changed,
        "a policy edit must not restart the watch"
    );

    // And stopping one frees the slot for a tenth section.
    committed(
        store
            .commit_desired_watch_mutation(&stop(section(2), revisions[1], 101))
            .unwrap(),
    );
    committed(
        store
            .commit_desired_watch_mutation(&start(section(10), 0, 102))
            .unwrap(),
    );
}

#[test]
fn a_policy_edit_keeps_the_epoch_and_a_desired_change_allocates_one() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let armed = committed(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
    );
    assert!(armed.epoch_changed);

    let mut edit = start(section(1), armed.revision, 2);
    edit.policy = Some(loud_policy());
    let edited = committed(store.commit_desired_watch_mutation(&edit).unwrap());
    assert!(
        edited.revision > armed.revision,
        "every commit advances the revision"
    );
    assert_eq!(
        edited.materialization_epoch, armed.materialization_epoch,
        "the materialization survives a policy edit"
    );
    assert!(!edited.epoch_changed);
    assert_eq!(
        store
            .desired_watch_authority()
            .unwrap()
            .counters
            .materialization_counter,
        armed.materialization_epoch,
        "an unchanged epoch must not consume a materialization number",
    );
    assert_eq!(
        store.desired_watches().unwrap(),
        vec![DesiredWatch {
            section: section(1),
            policy: loud_policy(),
        }],
    );
}

/// A STOP is unconditional. There is no state of the world in which the
/// authority may answer "no, you may not stop watching this" -- a watch the
/// user cannot turn off is worse than any silence -- so a stop passes the
/// generation, receipt and revision checks and nothing else. In particular it
/// skips the product cap: the cap is about how many sections may be watched,
/// and a stop only ever lowers that number.
#[test]
fn a_stop_is_never_refused_for_anything_but_a_stale_read() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let mut revisions = Vec::new();
    for index in 1..=MAX_DESIRED_WATCHES as u16 {
        revisions.push(
            committed(
                store
                    .commit_desired_watch_mutation(&start(section(index), 0, u64::from(index)))
                    .unwrap(),
            )
            .revision,
        );
    }
    assert_eq!(
        store.desired_watches().unwrap().len(),
        MAX_DESIRED_WATCHES,
        "the cap is full, which is exactly when a cap check would bite",
    );

    let stopped = committed(
        store
            .commit_desired_watch_mutation(&stop(section(1), revisions[0], 100))
            .unwrap(),
    );
    assert!(stopped.epoch_changed);
    assert_eq!(store.desired_watches().unwrap().len(), 8);

    // The only thing that can refuse a stop is a stale read of the row it
    // names, and that is the tombstone doing its job rather than a policy.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&stop(section(2), revisions[1] + 99, 101))
            .unwrap(),
        DesiredWatchMutationOutcome::StaleRevision {
            current: revisions[1]
        },
    );
}

#[test]
fn stopping_a_section_that_was_never_started_still_records_the_cancellation() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    // Reachable whenever a page cancels before its START commits. The row is
    // written rather than dropped so the revision line starts here: a START
    // still in flight carries `basedOnRevision = 0` and now fails, instead of
    // arriving after the cancellation and being admitted.
    let cancelled = committed(
        store
            .commit_desired_watch_mutation(&stop(section(1), 0, 1))
            .unwrap(),
    );
    assert!(
        cancelled.epoch_changed,
        "a new row has no earlier epoch to keep"
    );
    assert!(
        cancelled.materialization_epoch >= 1,
        "the stored epoch must satisfy the table bound"
    );
    let authority = store.desired_watch_authority().unwrap();
    assert_eq!(authority.entries.len(), 1);
    assert!(authority.entries[0].is_tombstone());
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 2))
            .unwrap(),
        DesiredWatchMutationOutcome::StaleRevision {
            current: cancelled.revision
        },
    );
}

#[test]
fn an_exhausted_authority_counter_is_an_error_rather_than_a_wrap() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let store = PersonalStateStore::open(&path).unwrap();
    drop(store);

    // Number.MAX_SAFE_INTEGER: every counter crosses the wire as a JavaScript
    // number, so the last usable value is the bound the table already CHECKs.
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE personal_state_metadata_v1
                SET desired_watch_revision_counter = 9007199254740991",
            [],
        )
        .unwrap();
    drop(connection);

    let mut store = PersonalStateStore::open(&path).unwrap();
    assert!(matches!(
        store.commit_desired_watch_mutation(&start(section(1), 0, 1)),
        Err(PersonalStateError::DesiredWatchCounterExhausted),
    ));
    assert_eq!(
        store.desired_watches().unwrap(),
        Vec::new(),
        "the refused mutation must leave nothing behind",
    );
    assert_eq!(receipt_count(&store), 0);
}

#[test]
fn the_persisted_receipt_format_is_pinned() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    // The fingerprint and the outcome JSON are ON-DISK format: a build from
    // next month reads rows this build wrote, and a silent change to either
    // one turns every stored receipt into a collision or a parse failure.
    // The fingerprint preimage is
    //     term ‖ 0 ‖ campus ‖ 0 ‖ index ‖ 0 ‖ basedOnRevision (8 bytes, BE)
    //     ‖ desired (1 byte) ‖ policy JSON (empty for a stop)
    // hashed with SHA-256; the digests below are for section T2026F/CAMPUS_A
    // with the default policy unless noted.
    //
    // Three outcome shapes exist and all three are pinned below: COMMITTED,
    // STALE_REVISION and LIMIT_EXCEEDED. Every one of them is a statement
    // about the AUTHORITY. A receipt never records a verdict about whether a
    // section can be watched, because that answer is allowed to change.
    let armed = committed(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
    );
    let mut edit = start(section(1), armed.revision, 2);
    edit.policy = Some(loud_policy());
    let edited = committed(store.commit_desired_watch_mutation(&edit).unwrap());
    committed(
        store
            .commit_desired_watch_mutation(&stop(section(1), edited.revision, 3))
            .unwrap(),
    );
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(2), 5, 4))
            .unwrap(),
        DesiredWatchMutationOutcome::StaleRevision { current: 0 },
    );
    for (mutation, fingerprint, outcome_json) in [
        (
            1_u64,
            "cae642bce43eec1880404ea32256675b44229ad55e10fcf25dbd0e3221bf6c5e",
            r#"{"outcome":"COMMITTED","revision":1,"materializationEpoch":1,"epochChanged":true}"#,
        ),
        (
            // Same section and base 1, but the loud policy, so a different
            // digest: the policy is part of what a retry must match.
            2,
            "a775f3a67283fb5e649a60cb07c6e0f71464ffc15aa06bd911118e8ec54050c6",
            r#"{"outcome":"COMMITTED","revision":2,"materializationEpoch":1,"epochChanged":false}"#,
        ),
        (
            // A stop: no policy in the preimage, and the desired byte is 0.
            3,
            "d0a86d1878f65ff95c6bdf6c4cf410bd2dd504f60fe0f4a551a9167ac25a98c3",
            r#"{"outcome":"COMMITTED","revision":3,"materializationEpoch":2,"epochChanged":true}"#,
        ),
        (
            4,
            "c4ff6d260d6a237ac376fd328e0b7a7d806467ee3f629d9af938ceaa6235868a",
            r#"{"outcome":"STALE_REVISION","current":0}"#,
        ),
    ] {
        let stored = stored_receipt(&path, mutation);
        assert_eq!(stored.0, fingerprint, "fingerprint for mutation {mutation}");
        assert_eq!(stored.1, outcome_json, "outcome for mutation {mutation}");
    }

    // The third outcome shape needs a full watch list of its own. There is no
    // fourth: the REJECTED shape the ledger used to carry held a verdict
    // about the section itself, and the writer no longer forms one.
    let capped = TempDir::new().unwrap();
    let capped_path = database_path(&capped);
    let mut capped_store = PersonalStateStore::open(&capped_path).unwrap();
    for index in 1..=MAX_DESIRED_WATCHES as u16 {
        committed(
            capped_store
                .commit_desired_watch_mutation(&start(section(index), 0, u64::from(index)))
                .unwrap(),
        );
    }
    assert_eq!(
        capped_store
            .commit_desired_watch_mutation(&start(section(10), 0, 10))
            .unwrap(),
        DesiredWatchMutationOutcome::LimitExceeded { maximum: 9 },
    );
    assert_eq!(
        stored_receipt(&capped_path, 10),
        (
            "66c7083385ca73da083d47194de98fe91ceccea283d171a7a953fec4d2a6c903".to_owned(),
            r#"{"outcome":"LIMIT_EXCEEDED","maximum":9}"#.to_owned(),
        ),
    );
}

#[test]
fn every_recorded_answer_survives_closing_and_reopening_the_database() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);

    let expected = {
        let mut store = PersonalStateStore::open(&path).unwrap();
        let mut expected = Vec::new();

        // Eight commits, leaving one slot so the stale-revision refusal below
        // is the refusal it claims to be rather than the cap firing first.
        for index in 1..=8_u16 {
            let commit = committed(
                store
                    .commit_desired_watch_mutation(&start(section(index), 0, u64::from(index)))
                    .unwrap(),
            );
            expected.push((
                start(section(index), 0, u64::from(index)),
                DesiredWatchReceiptOutcome::committed(commit),
            ));
        }

        let stale = start(section(20), 7, 20);
        assert_eq!(
            store.commit_desired_watch_mutation(&stale).unwrap(),
            DesiredWatchMutationOutcome::StaleRevision { current: 0 },
        );
        expected.push((
            stale,
            DesiredWatchReceiptOutcome::StaleRevision { current: 0 },
        ));

        // Now fill the last slot, so the tenth section meets the cap.
        let ninth = start(section(9), 0, 9);
        let commit = committed(store.commit_desired_watch_mutation(&ninth).unwrap());
        expected.push((ninth, DesiredWatchReceiptOutcome::committed(commit)));

        let capped = start(section(10), 0, 10);
        assert_eq!(
            store.commit_desired_watch_mutation(&capped).unwrap(),
            DesiredWatchMutationOutcome::LimitExceeded {
                maximum: MAX_DESIRED_WATCHES
            },
        );
        expected.push((
            capped,
            DesiredWatchReceiptOutcome::LimitExceeded {
                maximum: MAX_DESIRED_WATCHES,
            },
        ));
        expected
    };

    // A new process, a new connection, the same answers. Replaying from the
    // ledger is the only reason a client that lost its response can retry at
    // all, so it has to keep working across the restart that lost it.
    let mut reopened = PersonalStateStore::open(&path).unwrap();
    for (command, outcome) in &expected {
        assert_eq!(
            reopened
                .commit_desired_watch_mutation(command)
                .unwrap(),
            DesiredWatchMutationOutcome::Replayed(*outcome),
            "replay for {}",
            command.mutation_id,
        );
    }
    assert_eq!(
        receipt_count(&reopened),
        expected.len() as u64,
        "replaying inserts nothing",
    );
    assert_eq!(
        reopened.desired_watches().unwrap().len(),
        MAX_DESIRED_WATCHES,
    );
}

/// Bulk-seeds tombstones directly, because these budgets are only reachable
/// at a scale that would otherwise make the setup the slowest part of the
/// test. The rows are exactly what a stop writes.
fn seed_tombstones(path: &Path, count: u64) {
    seed_tombstones_from(path, 1, count);
}

fn seed_tombstones_from(path: &Path, first: u64, count: u64) {
    // The schema has to exist before rows can be put in it.
    drop(PersonalStateStore::open(path).unwrap());
    let connection = Connection::open(path).unwrap();
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    for index in first..first + count {
        connection
            .execute(
                "INSERT INTO personal_desired_watches_v1
                     (term_id, campus_code, section_index, desired, policy_json,
                      revision, materialization_epoch)
                 VALUES ('T2026F', 'CAMPUS_A', ?1, 0, NULL, ?2, ?2)",
                rusqlite::params![format!("{index:05}"), index as i64],
            )
            .unwrap();
    }
    connection
        .execute(
            "UPDATE personal_state_metadata_v1
                SET desired_watch_revision_counter = MAX(desired_watch_revision_counter, ?1),
                    desired_watch_materialization_counter =
                        MAX(desired_watch_materialization_counter, ?1)",
            [(first + count - 1) as i64],
        )
        .unwrap();
    connection.execute_batch("COMMIT").unwrap();
}

fn seed_receipts(path: &Path, count: u64) {
    seed_receipts_from(path, 1, count);
}

fn seed_receipts_from(path: &Path, first: u64, count: u64) {
    drop(PersonalStateStore::open(path).unwrap());
    let connection = Connection::open(path).unwrap();
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    for index in first..first + count {
        connection
            .execute(
                r#"INSERT INTO personal_desired_watch_receipts_v1
                     (authority_generation, mutation_id, term_id, campus_code,
                      section_index, fingerprint, outcome_json)
                 VALUES (1, ?1, 'T2026F', 'CAMPUS_A', '00001', ?2,
                         '{"outcome":"STALE_REVISION","current":0}')"#,
                rusqlite::params![trace(1_000_000 + index).to_string(), format!("{index:064}")],
            )
            .unwrap();
    }
    connection.execute_batch("COMMIT").unwrap();
}

#[test]
fn the_rotation_thresholds_are_the_frozen_eighty_percent_floors() {
    // Frozen values. They are pinned as literals rather than recomputed,
    // because a proof about the largest legal state is only as good as the
    // numbers it was made against.
    assert_eq!(MAX_DESIRED_WATCH_TOMBSTONES, 512);
    assert_eq!(MAX_DESIRED_WATCH_RECEIPTS, 2048);
    assert_eq!(DESIRED_WATCH_TOMBSTONE_ROTATION_THRESHOLD, 409);
    assert_eq!(DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD, 1638);
    // Floored, not rounded: 512 * 4 / 5 is 409.6 and 2048 * 4 / 5 is 1638.4.
    assert_eq!(DESIRED_WATCH_TOMBSTONE_ROTATION_THRESHOLD, 512 * 4 / 5);
    assert_eq!(DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD, 2048 * 4 / 5);
}

#[test]
fn the_budget_reader_composes_inside_a_callers_consistent_read() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let store = PersonalStateStore::open(&path).unwrap();

    // One statement, so it carries its own snapshot AND nests. Giving it a
    // transaction of its own would be the same regression that broke
    // `personal_table_counts` once already: a caller holding a snapshot
    // would get "cannot start a transaction within a transaction".
    let (budget, counts) = store
        .consistent_read(|store| {
            Ok((
                store.desired_watch_budget()?,
                store.personal_table_counts()?,
            ))
        })
        .unwrap();
    assert_eq!(budget.tombstones, 0);
    assert_eq!(budget.receipts, counts.desired_watch_receipts);
}

#[test]
fn the_tombstone_budget_signals_at_its_threshold_and_fails_closed_at_its_cap() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);

    seed_tombstones(&path, DESIRED_WATCH_TOMBSTONE_ROTATION_THRESHOLD - 1);
    let store = PersonalStateStore::open(&path).unwrap();
    assert_eq!(
        store.desired_watch_budget().unwrap().tombstones,
        DESIRED_WATCH_TOMBSTONE_ROTATION_THRESHOLD - 1,
    );
    assert!(!store.desired_watch_budget().unwrap().rotation_due());
    drop(store);

    seed_tombstones_from(&path, DESIRED_WATCH_TOMBSTONE_ROTATION_THRESHOLD, 1);
    let store = PersonalStateStore::open(&path).unwrap();
    assert!(
        store.desired_watch_budget().unwrap().rotation_due(),
        "rotation is due AT the threshold, not one past it",
    );
    drop(store);

    // Fill to the hard cap and give the user one armed section to stop.
    seed_tombstones_from(
        &path,
        DESIRED_WATCH_TOMBSTONE_ROTATION_THRESHOLD + 1,
        MAX_DESIRED_WATCH_TOMBSTONES - DESIRED_WATCH_TOMBSTONE_ROTATION_THRESHOLD,
    );
    let mut store = PersonalStateStore::open(&path).unwrap();
    assert_eq!(
        store.desired_watch_budget().unwrap().tombstones,
        MAX_DESIRED_WATCH_TOMBSTONES,
    );
    let armed = committed(
        store
            .commit_desired_watch_mutation(&start(section(9000), 0, 1))
            .unwrap(),
    );

    // The stop is refused, and refused NON-terminally: no receipt, nothing
    // written, and the section is still armed rather than reported stopped.
    let before = receipt_count(&store);
    assert_eq!(
        store
            .commit_desired_watch_mutation(&stop(section(9000), armed.revision, 2))
            .unwrap(),
        DesiredWatchMutationOutcome::AuthorityFull(DesiredWatchBudgetKind::Tombstones),
    );
    assert_eq!(
        receipt_count(&store),
        before,
        "a non-terminal answer is not recorded"
    );
    assert_eq!(
        store.desired_watch_budget().unwrap().tombstones,
        MAX_DESIRED_WATCH_TOMBSTONES,
        "the 513th tombstone must not land",
    );
    assert_eq!(store.desired_watches().unwrap().len(), 1);

    // Rotation is what unblocks it. The stop then goes through -- against the
    // new generation, because the old one is exactly what rotation retired.
    let rotation = store.rotate_desired_watch_authority(&BTreeSet::new()).unwrap();
    assert_eq!(rotation.deleted_tombstones, MAX_DESIRED_WATCH_TOMBSTONES);
    let mut retry = stop(section(9000), rotation.retained[0].revision, 3);
    retry.authority_generation = rotation.authority_generation;
    committed(store.commit_desired_watch_mutation(&retry).unwrap());
    assert_eq!(store.desired_watches().unwrap(), Vec::new());
}

#[test]
fn the_budget_reader_names_the_same_culprit_the_writer_does() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    seed_tombstones(&path, MAX_DESIRED_WATCH_TOMBSTONES);
    seed_receipts(&path, MAX_DESIRED_WATCH_RECEIPTS);
    let mut store = PersonalStateStore::open(&path).unwrap();

    // Both budgets full at once. The reader an actor consults and the writer
    // it is trying to unblock have to agree on which one is in the way;
    // disagreeing would send it rotating for a reason the store never gave.
    let budget = store.desired_watch_budget().unwrap();
    assert_eq!(budget.exhausted(), Some(DesiredWatchBudgetKind::Receipts));
    assert_eq!(
        store
            .commit_desired_watch_mutation(&stop(section(9000), 0, 1))
            .unwrap(),
        DesiredWatchMutationOutcome::AuthorityFull(DesiredWatchBudgetKind::Receipts),
    );
}

#[test]
fn the_receipt_budget_signals_at_its_threshold_and_fails_closed_at_its_cap() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);

    seed_receipts(&path, DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD - 1);
    let store = PersonalStateStore::open(&path).unwrap();
    assert!(!store.desired_watch_budget().unwrap().rotation_due());
    drop(store);

    seed_receipts_from(&path, DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD, 1);
    let store = PersonalStateStore::open(&path).unwrap();
    assert!(store.desired_watch_budget().unwrap().rotation_due());
    drop(store);

    seed_receipts_from(
        &path,
        DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD + 1,
        MAX_DESIRED_WATCH_RECEIPTS - DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD,
    );
    let mut store = PersonalStateStore::open(&path).unwrap();
    assert_eq!(
        store.desired_watch_budget().unwrap().receipts,
        MAX_DESIRED_WATCH_RECEIPTS,
    );

    // Refused before the command is decided at all. Deciding first and then
    // failing to record it would hand out an answer a retry could contradict.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
        DesiredWatchMutationOutcome::AuthorityFull(DesiredWatchBudgetKind::Receipts),
    );
    assert_eq!(
        store.desired_watch_budget().unwrap().receipts,
        MAX_DESIRED_WATCH_RECEIPTS,
        "the 2049th receipt must not land",
    );
    assert_eq!(store.desired_watches().unwrap(), Vec::new());

    let rotation = store.rotate_desired_watch_authority(&BTreeSet::new()).unwrap();
    assert_eq!(rotation.deleted_receipts, MAX_DESIRED_WATCH_RECEIPTS);
    let mut retry = start(section(1), 0, 1);
    retry.authority_generation = rotation.authority_generation;
    committed(store.commit_desired_watch_mutation(&retry).unwrap());
}

#[test]
fn a_replay_still_answers_when_the_ledger_is_full() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let first = committed(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
    );
    drop(store);
    seed_receipts_from(&path, 1, MAX_DESIRED_WATCH_RECEIPTS - 1);
    let mut store = PersonalStateStore::open(&path).unwrap();
    assert_eq!(
        store.desired_watch_budget().unwrap().receipts,
        MAX_DESIRED_WATCH_RECEIPTS,
    );

    // A client whose response was lost retries. It needs no new ledger row,
    // so a full ledger must not turn its recorded answer into "come back
    // later" -- that would be the authority forgetting something it knows.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
        DesiredWatchMutationOutcome::Replayed(DesiredWatchReceiptOutcome::committed(first)),
    );
}

/// Rotation collects the removal history EXCEPT the rows the caller says are
/// still being torn down.
///
/// Raising the generation is what makes a tombstone safe to drop as a
/// compare-and-swap guard, and that is the only thing it makes safe. A
/// tombstone whose section still has a physical watch nobody could stop is
/// also the row the read hangs `pendingDisarm` on -- drop it and a watch that
/// is still alive and can still ring is not on any page, in any list, or
/// behind any control. So the caller, which is the only thing that knows,
/// names them; they are carried over and renumbered like live intent.
#[test]
fn rotation_keeps_the_tombstones_the_caller_is_still_tearing_down() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let stuck = committed(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
    );
    let finished = committed(
        store
            .commit_desired_watch_mutation(&start(section(2), 0, 2))
            .unwrap(),
    );
    committed(
        store
            .commit_desired_watch_mutation(&start(section(3), 0, 3))
            .unwrap(),
    );
    committed(
        store
            .commit_desired_watch_mutation(&stop(section(1), stuck.revision, 4))
            .unwrap(),
    );
    committed(
        store
            .commit_desired_watch_mutation(&stop(section(2), finished.revision, 5))
            .unwrap(),
    );

    let preserved = BTreeSet::from([section(1)]);
    let rotation = store.rotate_desired_watch_authority(&preserved).unwrap();

    assert_eq!(
        rotation.deleted_tombstones, 1,
        "only the removal nothing is still working on is collected",
    );
    assert_eq!(
        rotation
            .retained
            .iter()
            .map(|entry| entry.section.clone())
            .collect::<Vec<_>>(),
        vec![section(1), section(3)],
        "the stuck removal survives beside the live intent",
    );
    let carried = rotation
        .retained
        .iter()
        .find(|entry| entry.section == section(1))
        .expect("the preserved row");
    assert!(carried.is_tombstone(), "it is still a removal, not intent");
    assert_eq!(
        (carried.revision, carried.materialization_epoch),
        (1, 1),
        "and it is renumbered into the new generation like anything else",
    );

    let after = store.desired_watch_authority().unwrap();
    assert_eq!(after.entries, rotation.retained, "the read agrees with it");
    assert_eq!(
        store.desired_watch_budget().unwrap().tombstones,
        1,
        "the preserved row is the only removal history left",
    );

    // It is still a real compare-and-swap guard in the new generation: a page
    // that reads nothing for the Section is refused rather than admitted.
    let mut resurrect = start(section(1), 0, 6);
    resurrect.authority_generation = rotation.authority_generation;
    assert_eq!(
        store.commit_desired_watch_mutation(&resurrect).unwrap(),
        DesiredWatchMutationOutcome::StaleRevision { current: 1 },
    );

    // Once the caller stops naming it, the next rotation collects it.
    let second = store.rotate_desired_watch_authority(&BTreeSet::new()).unwrap();
    assert_eq!(second.deleted_tombstones, 1);
    assert_eq!(
        second
            .retained
            .iter()
            .map(|entry| entry.section.clone())
            .collect::<Vec<_>>(),
        vec![section(3)],
    );
}

#[test]
fn rotation_carries_intent_into_a_new_generation_and_frees_both_budgets() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let armed = committed(
        store
            .commit_desired_watch_mutation(&start(section(1), 0, 1))
            .unwrap(),
    );
    committed(
        store
            .commit_desired_watch_mutation(&start(section(2), 0, 2))
            .unwrap(),
    );
    committed(
        store
            .commit_desired_watch_mutation(&stop(section(1), armed.revision, 3))
            .unwrap(),
    );
    // Give the incarnation a value only a running actor would have. Left at
    // the schema default this assertion compares 1 to 1 and would still pass
    // against a rotation that reset it.
    drop(store);
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE personal_state_metadata_v1 SET desired_watch_actor_incarnation = 7",
            [],
        )
        .unwrap();
    drop(connection);
    let mut store = PersonalStateStore::open(&path).unwrap();

    let before = store.desired_watch_authority().unwrap();
    assert_eq!(before.counters.authority_generation, 1);
    assert_eq!(before.counters.actor_incarnation, 7);
    assert_eq!(before.entries.len(), 2);

    let rotation = store.rotate_desired_watch_authority(&BTreeSet::new()).unwrap();
    assert_eq!(rotation.authority_generation, 2);
    assert_eq!(rotation.deleted_tombstones, 1);
    assert_eq!(rotation.deleted_receipts, 3);
    assert_eq!(
        rotation
            .retained
            .iter()
            .map(|e| e.section.clone())
            .collect::<Vec<_>>(),
        vec![section(2)],
        "only live intent survives; the removal history is what rotation is for",
    );
    assert_eq!(rotation.retained[0].revision, 1);
    assert_eq!(rotation.retained[0].materialization_epoch, 1);

    let after = store.desired_watch_authority().unwrap();
    assert_eq!(after.counters.authority_generation, 2);
    assert_eq!(after.counters.revision_counter, 1);
    assert_eq!(after.counters.materialization_counter, 1);
    assert_eq!(
        after.counters.actor_incarnation, 7,
        "rotation is authority maintenance, not a new actor",
    );
    assert_eq!(after.entries, rotation.retained);
    assert_eq!(store.desired_watch_budget().unwrap().tombstones, 0);
    assert_eq!(store.desired_watch_budget().unwrap().receipts, 0);

    // Anything still holding the old generation is refused at step one, which
    // is what makes renumbering inside the new generation safe.
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(3), 0, 4))
            .unwrap(),
        DesiredWatchMutationOutcome::StaleGeneration { current: 2 },
    );
}

#[test]
fn the_largest_legal_authority_state_is_bounded_by_the_two_caps() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    seed_tombstones(&path, MAX_DESIRED_WATCH_TOMBSTONES);
    let mut store = PersonalStateStore::open(&path).unwrap();
    for index in 1..=MAX_DESIRED_WATCHES as u16 {
        committed(
            store
                .commit_desired_watch_mutation(&start(section(9000 + index), 0, u64::from(index)))
                .unwrap(),
        );
    }

    // This is the number a frame-size proof has to be made against: every
    // section the product allows watched, plus a full removal history. It is
    // a bound only because the writer enforces both caps -- a reconciler that
    // skipped a round would otherwise leave it open-ended.
    let authority = store.desired_watch_authority().unwrap();
    assert_eq!(
        authority.entries.len() as u64,
        MAX_DESIRED_WATCH_AUTHORITY_ROWS,
    );
    assert_eq!(MAX_DESIRED_WATCH_AUTHORITY_ROWS, 521);
    assert_eq!(
        store
            .commit_desired_watch_mutation(&start(section(8000), 0, 100))
            .unwrap(),
        DesiredWatchMutationOutcome::LimitExceeded { maximum: 9 },
        "no tenth watch",
    );
    assert_eq!(
        store
            .commit_desired_watch_mutation(&stop(section(8000), 0, 101))
            .unwrap(),
        DesiredWatchMutationOutcome::AuthorityFull(DesiredWatchBudgetKind::Tombstones),
        "no 513th tombstone",
    );
    assert_eq!(store.desired_watch_authority().unwrap().entries.len(), 521);
}
