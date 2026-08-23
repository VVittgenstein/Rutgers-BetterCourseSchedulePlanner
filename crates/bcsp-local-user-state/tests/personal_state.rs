use std::num::NonZeroU64;
use std::path::Path;
use std::str::FromStr;

use bcsp_contracts::{
    FilterRequestV1, FilterValuesInputV1, NormalizedFilterValuesV1, OpenEpisodeId,
    OpenEpisodeState, SectionKey, TermId, TraceId, WatchContinuousDurationV1, WatchMaxAudible,
    WatchNotificationMode, WatchPolicyV1,
};
use bcsp_local_user_state::{
    CatalogRefreshMinutes, CurrentFiltersRevision, DesiredWatch,
    EpisodeActionInput, EpisodeActionKind,
    EpisodeDisposition, EpisodeHistoryIdentity, EpisodeSummaryInput, FilterAssociation,
    HistoryFilter, HistoryWriteOutcome, LocalSettings, LocaleOverride, MAX_SELECTED_SECTIONS,
    OpenRefreshSeconds, PageRequest, PersonalStateError, PersonalStateStore, SavedViewContent,
    SavedViewIncompatibility, SavedViewMatch, SavedViewRevision, SelectionMutation,
    SettingsRevision, UnixMillis, UserStateRevision, VolumePercent, WatchFastLaneSeconds,
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
fn two_openers_racing_the_same_pending_upgrade_both_start() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    drop(PersonalStateStore::open(&path).unwrap());

    // Rewind to 10003 so both openers see the same pending migration.
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
             ALTER TABLE personal_state_metadata_v1 RENAME TO personal_state_metadata_post;
             CREATE TABLE personal_state_metadata_v1 (
                 singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
                 state_revision INTEGER NOT NULL CHECK (state_revision > 0)
             ) STRICT;
             INSERT INTO personal_state_metadata_v1(singleton_id, state_revision)
                 SELECT singleton_id, state_revision FROM personal_state_metadata_post;
             DROP TABLE personal_state_metadata_post;",
        )
        .unwrap();
    drop(connection);

    // The prefix check runs before the write lock is taken, so both openers
    // can read "10003 applied". Only one may actually run 10004; the other
    // must notice inside the lock and start anyway.
    //
    // This is a REPRODUCER, not the guarantee: the two threads may serialize
    // so completely that the second never enters the window, in which case the
    // round passes vacuously. The guarantee is the re-read under the lock in
    // apply_migrations; the barrier below just makes the window likely to be
    // hit at least once.
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let path = path.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                PersonalStateStore::open(&path).map(|_| ())
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        handle.join().unwrap().expect("both openers start");
    }

    let store = PersonalStateStore::open(&path).unwrap();
    let migrations = store.migration_records().unwrap();
    assert_eq!(migrations.len(), 4, "10004 applied exactly once");
    assert_eq!(migrations[3].migration_id, 10_004);
}
