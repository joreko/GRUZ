CREATE TABLE IF NOT EXISTS source_stats (
    domain          TEXT    PRIMARY KEY,
    success_count   INTEGER NOT NULL DEFAULT 0,
    fail_count      INTEGER NOT NULL DEFAULT 0,
    -- JSON массив 24 элементов (байт/с по часам) или NULL
    avg_speed_by_hour TEXT,
    last_error      TEXT,
    last_seen       INTEGER,
    updated_at      INTEGER NOT NULL
);
