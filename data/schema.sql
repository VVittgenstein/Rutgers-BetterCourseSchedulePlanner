-- Reference tables
CREATE TABLE IF NOT EXISTS terms (
    term_id TEXT PRIMARY KEY,
    year INTEGER,
    term_code TEXT,
    display_name TEXT,
    start_on TEXT,
    end_on TEXT,
    snapshot_label TEXT
);

CREATE TABLE IF NOT EXISTS campuses (
    campus_code TEXT PRIMARY KEY,
    display_name TEXT,
    location_code TEXT,
    region TEXT
);

CREATE TABLE IF NOT EXISTS subjects (
    subject_code TEXT PRIMARY KEY,
    school_code TEXT,
    school_description TEXT,
    subject_description TEXT,
    campus_code TEXT,
    active INTEGER DEFAULT 1,
    FOREIGN KEY (campus_code) REFERENCES campuses(campus_code)
);

CREATE TABLE IF NOT EXISTS instructors (
    instructor_id TEXT PRIMARY KEY,
    full_name TEXT NOT NULL,
    normalized_name TEXT,
    source_payload TEXT
);

-- Course level entities
CREATE TABLE IF NOT EXISTS courses (
    course_id INTEGER PRIMARY KEY AUTOINCREMENT,
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    subject_code TEXT NOT NULL,
    course_number TEXT NOT NULL,
    course_string TEXT,
    title TEXT NOT NULL,
    expanded_title TEXT,
    level TEXT,
    credits_min REAL,
    credits_max REAL,
    credits_display TEXT,
    core_json TEXT,
    has_core_attribute INTEGER DEFAULT 0,
    prereq_html TEXT,
    prereq_plain TEXT,
    synopsis_url TEXT,
    course_notes TEXT,
    unit_notes TEXT,
    subject_notes TEXT,
    supplement_code TEXT,
    campus_locations_json TEXT,
    open_sections_count INTEGER,
    has_open_sections INTEGER DEFAULT 0,
    tags TEXT,
    search_vector TEXT,
    source_hash TEXT,
    source_payload TEXT,
    created_at TEXT,
    updated_at TEXT,
    FOREIGN KEY (term_id) REFERENCES terms(term_id),
    FOREIGN KEY (campus_code) REFERENCES campuses(campus_code),
    FOREIGN KEY (subject_code) REFERENCES subjects(subject_code)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_courses_term_campus_subject_number
    ON courses(term_id, campus_code, subject_code, course_number);

CREATE INDEX IF NOT EXISTS idx_courses_term_subject
    ON courses(term_id, campus_code, subject_code, title);

CREATE INDEX IF NOT EXISTS idx_courses_search_vector
    ON courses(term_id, campus_code, search_vector);

CREATE TABLE IF NOT EXISTS course_core_attributes (
    course_id INTEGER NOT NULL,
    term_id TEXT NOT NULL,
    core_code TEXT NOT NULL,
    reference_id TEXT,
    effective_term TEXT,
    metadata TEXT,
    PRIMARY KEY (course_id, core_code),
    FOREIGN KEY (course_id) REFERENCES courses(course_id) ON DELETE CASCADE,
    FOREIGN KEY (term_id) REFERENCES terms(term_id)
);

CREATE INDEX IF NOT EXISTS idx_core_code_course
    ON course_core_attributes(core_code, term_id);

-- Section level entities
CREATE TABLE IF NOT EXISTS sections (
    section_id INTEGER PRIMARY KEY AUTOINCREMENT,
    course_id INTEGER NOT NULL,
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    subject_code TEXT NOT NULL,
    section_number TEXT,
    index_number TEXT NOT NULL,
    open_status TEXT,
    is_open INTEGER DEFAULT 0,
    open_status_updated_at TEXT,
    instructors_text TEXT,
    section_notes TEXT,
    comments_json TEXT,
    eligibility_text TEXT,
    open_to_text TEXT,
    majors_json TEXT,
    minors_json TEXT,
    honor_programs_json TEXT,
    section_course_type TEXT,
    exam_code TEXT,
    exam_code_text TEXT,
    special_permission_add_code TEXT,
    special_permission_add_desc TEXT,
    special_permission_drop_code TEXT,
    special_permission_drop_desc TEXT,
    printed TEXT,
    session_print_indicator TEXT,
    subtitle TEXT,
    meeting_mode_summary TEXT,
    delivery_method TEXT,
    has_meetings INTEGER DEFAULT 0,
    source_hash TEXT,
    source_payload TEXT,
    created_at TEXT,
    updated_at TEXT,
    FOREIGN KEY (course_id) REFERENCES courses(course_id) ON DELETE CASCADE,
    FOREIGN KEY (term_id) REFERENCES terms(term_id),
    FOREIGN KEY (campus_code) REFERENCES campuses(campus_code),
    FOREIGN KEY (subject_code) REFERENCES subjects(subject_code)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_sections_term_index
    ON sections(term_id, index_number);

CREATE INDEX IF NOT EXISTS idx_sections_term_subject_status
    ON sections(term_id, campus_code, subject_code, is_open);

CREATE TABLE IF NOT EXISTS section_instructors (
    section_id INTEGER NOT NULL,
    instructor_id TEXT NOT NULL,
    display_order INTEGER,
    role TEXT,
    PRIMARY KEY (section_id, instructor_id),
    FOREIGN KEY (section_id) REFERENCES sections(section_id) ON DELETE CASCADE,
    FOREIGN KEY (instructor_id) REFERENCES instructors(instructor_id)
);

CREATE INDEX IF NOT EXISTS idx_instructors_name
    ON section_instructors(instructor_id, section_id);

CREATE TABLE IF NOT EXISTS section_meetings (
    meeting_id INTEGER PRIMARY KEY AUTOINCREMENT,
    section_id INTEGER NOT NULL,
    meeting_day TEXT,
    week_mask INTEGER,
    start_time_label TEXT,
    end_time_label TEXT,
    start_minutes INTEGER,
    end_minutes INTEGER,
    meeting_mode_code TEXT,
    meeting_mode_desc TEXT,
    campus_abbrev TEXT,
    campus_location_code TEXT,
    campus_location_desc TEXT,
    building_code TEXT,
    room_number TEXT,
    pm_code TEXT,
    ba_class_hours TEXT,
    online_only INTEGER DEFAULT 0,
    hash TEXT,
    FOREIGN KEY (section_id) REFERENCES sections(section_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_meetings_day_time
    ON section_meetings(week_mask, start_minutes, section_id);

CREATE TABLE IF NOT EXISTS section_populations (
    population_id INTEGER PRIMARY KEY AUTOINCREMENT,
    section_id INTEGER NOT NULL,
    population_type TEXT NOT NULL,
    code TEXT,
    is_unit_code INTEGER,
    is_major_code INTEGER,
    raw_payload TEXT,
    FOREIGN KEY (section_id) REFERENCES sections(section_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_populations_code
    ON section_populations(population_type, code);

CREATE TABLE IF NOT EXISTS section_crosslistings (
    section_id INTEGER NOT NULL,
    related_index TEXT NOT NULL,
    related_subject_code TEXT,
    related_course_number TEXT,
    raw_payload TEXT,
    PRIMARY KEY (section_id, related_index),
    FOREIGN KEY (section_id) REFERENCES sections(section_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS section_status_events (
    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    section_id INTEGER NOT NULL,
    previous_status TEXT,
    current_status TEXT,
    source TEXT,
    snapshot_term TEXT,
    snapshot_campus TEXT,
    snapshot_received_at TEXT,
    FOREIGN KEY (section_id) REFERENCES sections(section_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_status_events_section
    ON section_status_events(section_id, snapshot_received_at);

-- Subscription + notification tables
CREATE TABLE IF NOT EXISTS subscriptions (
    subscription_id INTEGER PRIMARY KEY AUTOINCREMENT,
    section_id INTEGER,
    term_id TEXT,
    campus_code TEXT,
    index_number TEXT,
    contact_type TEXT NOT NULL,
    contact_value TEXT NOT NULL,
    contact_hash TEXT NOT NULL,
    locale TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    is_verified INTEGER DEFAULT 0,
    created_at TEXT,
    updated_at TEXT,
    last_notified_at TEXT,
    last_known_section_status TEXT,
    unsubscribe_token TEXT,
    metadata TEXT,
    FOREIGN KEY (section_id) REFERENCES sections(section_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_subscriptions_active_unique
    ON subscriptions(section_id, contact_hash, contact_type)
    WHERE status IN ('pending', 'active');

CREATE INDEX IF NOT EXISTS idx_subscriptions_active
    ON subscriptions(section_id, status);

CREATE TABLE IF NOT EXISTS subscription_events (
    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    subscription_id INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    section_status_snapshot TEXT,
    payload TEXT,
    created_at TEXT,
    FOREIGN KEY (subscription_id) REFERENCES subscriptions(subscription_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS open_section_snapshots (
    snapshot_id INTEGER PRIMARY KEY AUTOINCREMENT,
    term_id TEXT NOT NULL,
    campus_code TEXT NOT NULL,
    index_number TEXT NOT NULL,
    seen_open_at TEXT,
    source_hash TEXT
);

CREATE INDEX IF NOT EXISTS idx_open_section_snapshot
    ON open_section_snapshots(term_id, campus_code, index_number);

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

-- Search helper
CREATE VIRTUAL TABLE IF NOT EXISTS course_search_fts
USING fts5(term_id, campus_code, course_id, section_id, document, tokenize = 'porter');
