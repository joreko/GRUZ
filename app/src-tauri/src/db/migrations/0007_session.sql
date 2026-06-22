CREATE TABLE IF NOT EXISTS session (
    id          INTEGER PRIMARY KEY DEFAULT 1,
    last_url    TEXT,
    last_dir    TEXT,
    window_x    INTEGER,
    window_y    INTEGER,
    window_w    INTEGER,
    window_h    INTEGER,
    updated_at  INTEGER NOT NULL
);

INSERT OR IGNORE INTO session (id, updated_at) VALUES (1, unixepoch());
