-- Smriti library schema v1.
-- Originals are never written to; everything the app learns lives here.

CREATE TABLE sources (
    id            INTEGER PRIMARY KEY,
    path          TEXT    NOT NULL UNIQUE,
    volume_id     TEXT,                       -- disk serial: survives drive-letter changes
    include_globs TEXT    NOT NULL DEFAULT '[]',
    exclude_globs TEXT    NOT NULL DEFAULT '[]',
    is_online     INTEGER NOT NULL DEFAULT 1,
    added_at      TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_scan_at  TEXT
);

CREATE TABLE places (
    id       INTEGER PRIMARY KEY,
    name     TEXT NOT NULL,
    admin1   TEXT,
    country  TEXT,
    lat      REAL NOT NULL,
    lon      REAL NOT NULL,
    UNIQUE (name, admin1, country)
);

CREATE TABLE trips (
    id               INTEGER PRIMARY KEY,
    title            TEXT NOT NULL,
    start_date       TEXT NOT NULL,
    end_date         TEXT NOT NULL,
    home_distance_km REAL,
    stops            TEXT NOT NULL DEFAULT '[]',  -- JSON [{place, lat, lon, start, end}]
    user_edited      INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE assets (
    id              INTEGER PRIMARY KEY,
    source_id       INTEGER NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
    rel_path        TEXT    NOT NULL,
    size            INTEGER NOT NULL,
    mtime           INTEGER NOT NULL,           -- unix seconds
    fingerprint     TEXT,                       -- XXH3(size + head + tail + exif date)
    kind            TEXT    NOT NULL CHECK (kind IN ('photo', 'video')),
    mime            TEXT,
    width           INTEGER,
    height          INTEGER,
    duration_ms     INTEGER,
    taken_at        TEXT,                       -- local wall-clock time, ISO 8601
    taken_at_source TEXT CHECK (taken_at_source IN ('exif', 'video', 'filename', 'mtime', 'user')),
    tz_offset_min   INTEGER,
    lat             REAL,
    lon             REAL,
    place_id        INTEGER REFERENCES places(id),
    trip_id         INTEGER REFERENCES trips(id) ON DELETE SET NULL,
    camera          TEXT,
    orientation     INTEGER,
    quality         REAL,
    burst_group     INTEGER,
    is_favorite     INTEGER NOT NULL DEFAULT 0,
    is_hidden       INTEGER NOT NULL DEFAULT 0,
    status          TEXT NOT NULL DEFAULT 'active'
                    CHECK (status IN ('active', 'missing', 'offline', 'deleted')),
    indexed_at      TEXT,
    UNIQUE (source_id, rel_path)
);
CREATE INDEX assets_taken_at    ON assets (taken_at);
CREATE INDEX assets_fingerprint ON assets (fingerprint);
CREATE INDEX assets_place       ON assets (place_id);
CREATE INDEX assets_trip        ON assets (trip_id);
CREATE INDEX assets_status      ON assets (status, kind);

CREATE TABLE persons (
    id            INTEGER PRIMARY KEY,
    name          TEXT,
    birth_date    TEXT,                         -- powers age-aware matching for children
    cover_face_id INTEGER,
    is_family     INTEGER NOT NULL DEFAULT 0,
    hidden        INTEGER NOT NULL DEFAULT 0,
    color         TEXT,
    created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE faces (
    id           INTEGER PRIMARY KEY,
    asset_id     INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    frame_ms     INTEGER,                       -- video frame time; NULL for photos
    bbox         TEXT NOT NULL,                 -- JSON [x, y, w, h] relative to frame
    landmarks    TEXT,                          -- JSON 5 points
    det_score    REAL NOT NULL,
    quality      REAL,
    embedding    BLOB,
    cluster_id   INTEGER,
    person_id    INTEGER REFERENCES persons(id) ON DELETE SET NULL,
    assign_kind  TEXT CHECK (assign_kind IN ('auto', 'confirmed', 'rejected')),
    assign_score REAL
);
CREATE INDEX faces_asset   ON faces (asset_id);
CREATE INDEX faces_person  ON faces (person_id, asset_id);
CREATE INDEX faces_cluster ON faces (cluster_id);

CREATE TABLE person_prototypes (
    person_id INTEGER NOT NULL REFERENCES persons(id) ON DELETE CASCADE,
    age_band  TEXT    NOT NULL,                 -- '0-1', '1-2', '2-4', '4-7', '7-12', '12+', 'adult'
    embedding BLOB    NOT NULL,
    n_faces   INTEGER NOT NULL,
    PRIMARY KEY (person_id, age_band)
);

CREATE TABLE albums (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    kind       TEXT NOT NULL CHECK (kind IN ('manual', 'smart')),
    query      TEXT,                            -- JSON Query for smart albums
    cover_id   INTEGER REFERENCES assets(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE album_items (
    album_id INTEGER NOT NULL REFERENCES albums(id) ON DELETE CASCADE,
    asset_id INTEGER NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (album_id, asset_id)
);

CREATE TABLE jobs (
    id         INTEGER PRIMARY KEY,
    asset_id   INTEGER REFERENCES assets(id) ON DELETE CASCADE,
    stage      TEXT    NOT NULL,                -- 'metadata', 'thumbnail', 'faces', ...
    priority   INTEGER NOT NULL DEFAULT 100,    -- lower runs first
    attempts   INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    state      TEXT    NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'running', 'failed')),
    updated_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (asset_id, stage)
);
CREATE INDEX jobs_queue ON jobs (state, priority, id);

CREATE TABLE events_log (
    id      INTEGER PRIMARY KEY,
    ts      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    kind    TEXT NOT NULL,
    payload TEXT NOT NULL
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
