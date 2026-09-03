use bcsp_contracts::{WatchClientCommandV1, WatchServerEventV1};

#[test]
fn malformed_or_ambiguous_client_watch_commands_fail_closed() {
    let invalid = [
        r#"{"type":"START_WATCH","items":[]}"#,
        r#"{"type":"ACKNOWLEDGE_ALL_EPISODES","future":true}"#,
        r#"{"type":"STOP_WATCH","watch":{"activeWatchId":"00000000-0000-4000-8000-000000000002","sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"},"future":true}}"#,
        r#"{"type":"START_WATCH","items":[{"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"},"policy":{"notificationMode":"ONE_SHOT","maxAudible":0,"continuousDuration":{"kind":"FINITE","seconds":600}}}]}"#,
        r#"{"type":"START_WATCH","items":[{"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"},"policy":{"notificationMode":"ONE_SHOT","maxAudible":3,"continuousDuration":{"kind":"FINITE","seconds":0}}}]}"#,
        r#"{"type":"START_WATCH","items":[{"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"},"policy":{"notificationMode":"ONE_SHOT","maxAudible":9007199254740992,"continuousDuration":{"kind":"FINITE","seconds":600}}}]}"#,
        r#"{"type":"START_WATCH","items":[{"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"},"policy":{"notificationMode":"ONE_SHOT","maxAudible":3,"continuousDuration":{"kind":"FINITE","seconds":9007199254740992}}}]}"#,
        r#"{"type":"FUTURE_COMMAND"}"#,
        r#"{"type":"HEARTBEAT_ACK"}"#,
        r#"{"type":"HEARTBEAT_ACK","sequence":1,"future":true}"#,
    ];
    for json in invalid {
        assert!(
            serde_json::from_str::<WatchClientCommandV1>(json).is_err(),
            "{json}"
        );
    }
}

#[test]
fn server_watch_projection_accepts_additive_fields() {
    let json = r#"{
      "type":"START_RESULT",
      "result":{
        "contractVersion":1,
        "items":[{
          "status":"REJECTED",
          "sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"},
          "reason":"MAX_ACTIVE_WATCHES",
          "futureItemFact":true
        }],
        "activeWatchCount":0,
        "futureResultFact":true
      },
      "futureEventFact":true
    }"#;
    let decoded = serde_json::from_str::<WatchServerEventV1>(json).unwrap();
    assert!(matches!(decoded, WatchServerEventV1::StartResult { .. }));
}

#[test]
fn invalid_episode_state_and_timeline_are_rejected_at_decode() {
    let golden = include_str!("golden/watch-server-episode-v1.json");
    let mut value: serde_json::Value = serde_json::from_str(golden).unwrap();
    value["episode"]["closedAt"] = serde_json::json!("1970-01-01T00:00:02Z");
    assert!(serde_json::from_value::<WatchServerEventV1>(value).is_err());

    let mut value: serde_json::Value = serde_json::from_str(golden).unwrap();
    value["episode"]["observationCount"] = serde_json::json!(0);
    assert!(serde_json::from_value::<WatchServerEventV1>(value).is_err());
}
