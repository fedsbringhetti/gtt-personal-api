-- Rules (Phase 3, section 5): scheduled summaries and trip trackings.
CREATE TABLE rules (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('summary', 'track')),
    enabled       INTEGER NOT NULL DEFAULT 1,
    stop_id       TEXT NOT NULL,        -- gtfsId; no foreign key, the stop may disappear after a sync
    days          TEXT NOT NULL,        -- JSON: ["mon", ...]; empty for one-off trackings
    date          TEXT,                 -- one-off trackings only, YYYY-MM-DD
    skip_holidays INTEGER NOT NULL DEFAULT 1,
    paused_until  TEXT,                 -- the last day skipped, included (decision AH)
    config        TEXT NOT NULL,        -- JSON: the fields of the kind, with their defaults
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
