CREATE TABLE IF NOT EXISTS history (
    id              TEXT    PRIMARY KEY,
    url             TEXT    NOT NULL,
    video_id        TEXT,                         -- платформенный ID видео (yt: 11 chars)
    platform        TEXT    NOT NULL DEFAULT '',  -- 'youtube' | 'vk' | 'tiktok' | ...
    title           TEXT    NOT NULL,
    channel         TEXT,
    channel_id      TEXT,                         -- стабильный ID канала
    thumbnail       TEXT,
    duration        INTEGER,                      -- секунды
    file_path       TEXT    NOT NULL,
    file_size       INTEGER,
    format          TEXT    NOT NULL,             -- 'video' | 'audio' | 'video_only'
    quality         TEXT    NOT NULL,
    container       TEXT    NOT NULL,
    fps             INTEGER,
    bitrate         INTEGER,
    audio_codec     TEXT,
    video_codec     TEXT,
    trim_start      INTEGER,                      -- секунды | NULL
    trim_end        INTEGER,                      -- секунды | NULL
    playlist_id     TEXT,
    playlist_index  INTEGER,
    created_at      INTEGER NOT NULL              -- unixepoch()
);

CREATE INDEX idx_history_created   ON history(created_at DESC);
CREATE INDEX idx_history_video_id  ON history(video_id)    WHERE video_id  IS NOT NULL;
CREATE INDEX idx_history_channel   ON history(channel_id)  WHERE channel_id IS NOT NULL;
CREATE INDEX idx_history_platform  ON history(platform)    WHERE platform != '';
CREATE INDEX idx_history_playlist  ON history(playlist_id) WHERE playlist_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS settings (
    key         TEXT    PRIMARY KEY,
    value       TEXT    NOT NULL,
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Дефолтные настройки (все ключи)
INSERT OR IGNORE INTO settings (key, value) VALUES
    -- загрузка
    ('download_dir',         ''),
    ('max_concurrent',       '3'),
    ('proxy',                ''),
    ('ytdlp_extra_args',     ''),
    ('ytdlp_auto_update',    'true'),
    -- форматы
    ('default_format',       'video'),
    ('default_quality',      'best'),
    ('default_container',    'mp4'),
    ('auto_merge',           'true'),
    ('embed_subtitles',      'false'),
    -- папки и шаблоны
    ('save_dir_video',       ''),
    ('save_dir_audio',       ''),
    ('save_dir_playlist',    ''),
    ('save_dir_shorts',      ''),
    ('save_dir_trimmed',     ''),
    ('save_tpl_video',       '%(title)s.%(ext)s'),
    ('save_tpl_audio',       '%(title)s.%(ext)s'),
    ('save_tpl_playlist',    '%(playlist_title)s/%(playlist_index)s - %(title)s.%(ext)s'),
    ('save_tpl_shorts',      'Shorts/%(title)s.%(ext)s'),
    ('save_tpl_trimmed',     '%(title)s [trimmed].%(ext)s'),
    -- UI
    ('theme',                'dark'),
    ('minimize_to_tray',     'true'),
    -- HTTP API
    ('http_api_enabled',     'false'),
    ('http_api_port',        '7765');
