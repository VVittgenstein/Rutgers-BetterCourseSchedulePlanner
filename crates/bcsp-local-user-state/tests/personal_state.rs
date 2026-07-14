use std::num::NonZeroU64;
use std::path::Path;
use std::str::FromStr;

use bcsp_contracts::{
    FilterRequestV1, FilterValuesInputV1, NormalizedFilterValuesV1, OpenEpisodeId,
    OpenEpisodeState, SectionKey, TermId, TraceId, WatchContinuousDurationV1, WatchMaxAudible,
    WatchNotificationMode, WatchPolicyV1,
};
use bcsp_local_user_state::{
    CatalogRefreshMinutes, CurrentFilters, EpisodeActionInput, EpisodeActionKind,
    EpisodeDisposition, EpisodeHistoryIdentity, EpisodeSummaryInput, FilterAssociation,
    HistoryFilter, HistoryWriteOutcome, LocalSettings, LocaleOverride, MAX_SELECTED_SECTIONS,
    OpenRefreshSeconds, PageRequest, PersonalStateError, PersonalStateStore, SelectionMutation,
    SettingsRevision, UnixMillis, VolumePercent,
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
    let input = FilterValuesInputV1::for_term(TermId::try_from("T2026F").unwrap());
    FilterRequestV1::new(NormalizedFilterValuesV1::try_new(input).unwrap())
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
    assert_eq!(migrations.len(), 1);
    assert_eq!(migrations[0].migration_id, 10_001);
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
        volume_percent: VolumePercent::try_from(73).unwrap(),
        sound_policy: WatchPolicyV1::new(
            WatchNotificationMode::Continuous,
            WatchMaxAudible::try_from(9).unwrap(),
            WatchContinuousDurationV1::finite_seconds(600).unwrap(),
        ),
        current_filters: Some(CurrentFilters::new(
            FilterAssociation::Applied {
                view_id: trace(30),
                revision: 7,
            },
            filters(),
        )),
    };
    let stored = store
        .compare_and_swap_settings(SettingsRevision::ZERO, &settings)
        .unwrap();
    assert_eq!(stored.revision.get(), 1);
    assert!(matches!(
        store.compare_and_swap_settings(SettingsRevision::ZERO, &settings),
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
fn selection_persists_order_and_the_tenth_section_is_explicitly_rejected() {
    let directory = TempDir::new().unwrap();
    let path = database_path(&directory);
    let mut store = PersonalStateStore::open(&path).unwrap();
    for value in 1..=MAX_SELECTED_SECTIONS as u16 {
        assert_eq!(
            store.add_selected_section(section(value)).unwrap(),
            SelectionMutation::Added {
                position: (value - 1) as u8
            }
        );
    }
    assert!(matches!(
        store.add_selected_section(section(10)),
        Err(PersonalStateError::SelectionLimitExceeded { maximum: 9 })
    ));
    assert!(matches!(
        store.add_selected_section(section(1)).unwrap(),
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
        .compare_and_swap_settings(SettingsRevision::ZERO, &settings)
        .unwrap();
    store
        .replace_selected_sections(&[section(1), section(2)])
        .unwrap();
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
    assert_eq!(snapshot.episode_history.total, 1);
    let reset = store.clear_personal_data().unwrap();
    assert_eq!(reset.deleted_settings, 1);
    assert_eq!(reset.deleted_selected_sections, 2);
    assert_eq!(reset.deleted_episode_summaries, 1);
    assert_eq!(reset.deleted_episode_actions, 1);
    assert_eq!(store.personal_table_counts().unwrap(), Default::default());
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
        1
    );
    let serialized = serde_json::to_value(snapshot).unwrap();
    assert_eq!(serialized["activeWatchCount"], json!(0));
}
