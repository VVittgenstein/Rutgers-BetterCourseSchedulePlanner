-- Tables to capture open section events and notification fan-out queue
CREATE TABLE IF NOT EXISTS open_events (
    open_event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    section_id INTEGER,
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    index_number TEXT NOT NULL,
    status_before TEXT,
    status_after TEXT NOT NULL,
    seat_delta INTEGER,
    event_at TEXT NOT NULL,
    detected_by TEXT NOT NULL,
    snapshot_id INTEGER,
    dedupe_key TEXT NOT NULL,
    trace_id TEXT,
    payload TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (section_id) REFERENCES sections(section_id) ON DELETE SET NULL,
    FOREIGN KEY (snapshot_id) REFERENCES open_section_snapshots(snapshot_id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_open_events_lookup
    ON open_events(term_id, campus_code, index_number);

CREATE INDEX IF NOT EXISTS idx_open_events_dedupe
    ON open_events(dedupe_key, event_at DESC);

CREATE TABLE IF NOT EXISTS open_event_notifications (
    notification_id INTEGER PRIMARY KEY AUTOINCREMENT,
    open_event_id INTEGER NOT NULL,
    subscription_id INTEGER NOT NULL,
    dedupe_key TEXT NOT NULL,
    fanout_status TEXT NOT NULL DEFAULT 'pending',
    fanout_attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at TEXT,
    locked_by TEXT,
    locked_at TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (open_event_id) REFERENCES open_events(open_event_id) ON DELETE CASCADE,
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(subscription_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_open_event_notifications_event_sub
    ON open_event_notifications(open_event_id, subscription_id);

CREATE INDEX IF NOT EXISTS idx_open_event_notifications_status
    ON open_event_notifications(fanout_status, subscription_id);
