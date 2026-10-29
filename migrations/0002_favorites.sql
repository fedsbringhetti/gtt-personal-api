-- Favorite stops, in a manual order chosen by the user.
CREATE TABLE favorites (
    id         INTEGER PRIMARY KEY,
    stop_id    TEXT NOT NULL,        -- gtfsId; no foreign key, the stop may disappear after a sync
    label      TEXT,                 -- e.g. "Home → work"
    lines      TEXT NOT NULL,        -- JSON array of line names; empty means all lines
    position   INTEGER NOT NULL,     -- 0-based, kept contiguous
    created_at TEXT NOT NULL
);

CREATE INDEX favorites_position ON favorites (position);
