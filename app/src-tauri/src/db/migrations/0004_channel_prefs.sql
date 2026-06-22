CREATE TABLE IF NOT EXISTS channel_prefs (
    channel_id      TEXT    PRIMARY KEY,
    channel_name    TEXT    NOT NULL DEFAULT '',
    platform        TEXT    NOT NULL DEFAULT '',
    format          TEXT,
    quality         TEXT,
    container       TEXT,
    audio_codec     TEXT,
    video_codec     TEXT,
    fps             INTEGER,
    download_dir    TEXT,
    updated_at      INTEGER NOT NULL
);
