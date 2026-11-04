-- GTT's stop ids and ours, from stops.txt in GTT's GTFS static archive (Phase 3, decision Q).
-- Alerts name stops by GTT's stop_id (2843); our id comes from its stop_code (gtt:39).
-- Replaced in full at each download.
CREATE TABLE gtt_stops (
    gtt_stop_id TEXT PRIMARY KEY,   -- GTT stop_id, e.g. "2843"
    stop_id     TEXT NOT NULL       -- our gtfsId, e.g. "gtt:39"
);

-- The version of the data behind a sync_state row, when the source has one: for "gtt_static",
-- the archive's Last-Modified, sent back as If-Modified-Since at the next check.
ALTER TABLE sync_state ADD COLUMN version TEXT;
