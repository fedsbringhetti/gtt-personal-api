-- Local copy of all upstream stops, replaced in full by every sync.
CREATE TABLE stops (
    id          TEXT PRIMARY KEY,   -- gtfsId, e.g. "gtt:39"
    feed        TEXT NOT NULL,      -- "gtt", "2", "3", "4"
    code        TEXT NOT NULL,      -- code shown to the user
    name        TEXT NOT NULL,      -- original name
    description TEXT,
    lat         REAL NOT NULL,
    lon         REAL NOT NULL,
    mode        TEXT,               -- BUS, TRAM, SUBWAY, RAIL, FUNICULAR
    lines       TEXT NOT NULL,      -- JSON: [{"id": "...", "name": "4", "mode": "TRAM"}]
    search_name TEXT NOT NULL,      -- normalized name, used for ranking
    search_text TEXT NOT NULL       -- normalized name + description, used for matching
);

CREATE INDEX stops_code ON stops (code COLLATE NOCASE);
CREATE INDEX stops_lat_lon ON stops (lat, lon);

-- Full-text index on search_text; trigrams also match partial words.
CREATE VIRTUAL TABLE stops_fts USING fts5 (id UNINDEXED, text, tokenize = 'trigram');

CREATE TABLE sync_state (
    key   TEXT PRIMARY KEY,         -- e.g. "stops"
    at    TEXT NOT NULL,            -- last successful sync, RFC 3339
    count INTEGER NOT NULL
);
