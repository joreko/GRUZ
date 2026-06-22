CREATE TABLE IF NOT EXISTS quality_profiles (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT    NOT NULL UNIQUE,
    format      TEXT    NOT NULL,
    quality     TEXT    NOT NULL,
    container   TEXT    NOT NULL,
    audio_codec TEXT,
    video_codec TEXT,
    fps         INTEGER,
    bitrate     INTEGER,
    is_default  INTEGER NOT NULL DEFAULT 0,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL
);
