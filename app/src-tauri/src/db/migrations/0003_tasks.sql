CREATE TABLE IF NOT EXISTS tasks (
    id              TEXT    PRIMARY KEY,          -- совпадает с DownloadTask.id
    url             TEXT    NOT NULL,
    video_id        TEXT,
    platform        TEXT    NOT NULL DEFAULT '',
    title           TEXT,
    channel         TEXT,
    channel_id      TEXT,
    thumbnail       TEXT,
    duration        INTEGER,
    format          TEXT    NOT NULL,
    quality         TEXT    NOT NULL,
    container       TEXT    NOT NULL,
    fps             INTEGER,
    bitrate         INTEGER,
    audio_codec     TEXT,
    video_codec     TEXT,
    trim_start      INTEGER,
    trim_end        INTEGER,
    is_playlist     INTEGER NOT NULL DEFAULT 0,
    playlist_id     TEXT,
    playlist_index  INTEGER,
    state           TEXT    NOT NULL DEFAULT 'waiting',
    priority        INTEGER NOT NULL DEFAULT 1,
    progress        REAL    NOT NULL DEFAULT 0.0,
    error           TEXT,
    file_path       TEXT,
    file_size       INTEGER,
    schedule_at     INTEGER,                      -- NULL = немедленно
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE INDEX idx_tasks_state    ON tasks(state);
CREATE INDEX idx_tasks_priority ON tasks(priority DESC, created_at ASC)
    WHERE state IN ('waiting', 'paused');
CREATE INDEX idx_tasks_schedule ON tasks(schedule_at)
    WHERE schedule_at IS NOT NULL;
