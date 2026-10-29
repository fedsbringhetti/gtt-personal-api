# Phase 1 — Core backend: detailed plan

Status: **approved**, decisions confirmed on 2026-10-29. Development not started yet.

## 1. Goal

An HTTP API, running in Docker on the Orange Pi 5, that can:
- search for a stop by name, by code, or by location;
- show the next departures from a stop, filtered by line;
- store favorite stops.

### Definition of done

- [ ] Searching `porta nuova` returns GTT stops 39, 40 and 253 among the first results.
- [ ] Searching `39` returns stop 39 as the first result.
- [ ] Departures for stop 39 match those shown on muoversiatorino.it.
- [ ] Times stay correct after midnight and across daylight saving changes (covered by tests).
- [ ] If the upstream is down, stop search still works and departures return a clear error.
- [ ] The container runs on the Orange Pi, survives a restart without losing data, and uses less than 50 MB of RAM at idle.

## 2. Decisions

| # | Question | Decision |
|---|----------|----------|
| A | Which feeds to index? | All four (about 19,400 stops, 5.7 MB, downloaded in 2 seconds); filtering happens at search time, defaulting to `gtt` only |
| B | Authentication | No login; optional token (`API_TOKEN`) required when set |
| C | Users | Single user: favorites are not tied to an account |
| D | Delay history | Not in this phase; the schema does not rule it out and it will be revisited in Phase 2 |
| E | Compatible `/query.php?stop=` endpoint | No |
| F | How the image gets to the Orange Pi | Built on the Mac (`linux/arm64`, native on Apple Silicon) and pushed to GitHub Container Registry |

## 3. What we know about the upstream

Verified on 2026-10-29 against `https://plan.muoversiatorino.it/otp/routers/mato/index/graphql`.

**Feeds**
| Feed | Content | Stops |
|------|---------|-------|
| `gtt` | GTT urban, suburban, rail (SFM), tourist | 7,058 |
| `2` | Trenitalia | 447 |
| `3` | Arriva Italia | 1,358 |
| `4` | Regional bus companies (GrandaBus, SAV, etc.) | 10,559 |

**Quirks**
- For GTT the stop code matches the part after `gtt:`, except for the 23 metro stops (`gtt:8213`–`gtt:8227`), which have `code: null`. The id is used as a fallback code.
- Codes repeat across feeds (1,523 duplicates), so the internal key is always the full `gtfsId`.
- Names are uppercase with abbreviations (`IVREA (V. CASALE/PONTE BREDA)`, `C.SO`, `P.ZA`): they must be normalized for search.
- `stops(name: ...)` always returns an empty list: name search must be done locally.
- Times are seconds from `serviceDay`, a Unix timestamp marking the start of the service day. They can exceed 86,400 for trips after midnight.
- Unknown stop: `{"data":{"stop":null}}` with HTTP 200.
- Invalid query: HTTP 500 with an `errors` array.
- No rate limit or cache headers in responses.

## 4. Project structure

A single Rust crate.

```
gtt-personal-api/
├── Cargo.toml
├── migrations/               # SQL migrations (sqlx)
├── src/
│   ├── main.rs               # startup, wiring, graceful shutdown
│   ├── config.rs             # configuration from environment variables
│   ├── db.rs                 # SQLite pool and migrations
│   ├── time.rs               # serviceDay + seconds → Europe/Rome time
│   ├── upstream/
│   │   ├── mod.rs            # HTTP client, timeouts, retry, error handling
│   │   ├── queries.rs        # GraphQL queries
│   │   └── types.rs          # response types
│   ├── stops/
│   │   ├── sync.rs           # index download and refresh
│   │   ├── normalize.rs      # name normalization
│   │   └── search.rs         # search by text, code and location
│   ├── departures.rs         # departures and cache
│   ├── favorites.rs
│   └── api/
│       ├── mod.rs            # router
│       ├── stops.rs
│       ├── favorites.rs
│       ├── health.rs
│       └── error.rs          # JSON error format
├── tests/
│   └── fixtures/             # recorded upstream responses
├── Dockerfile
└── docker-compose.yml
```

**Main dependencies**: `axum`, `tokio`, `reqwest` (with `rustls`, no OpenSSL), `sqlx` (SQLite), `serde`, `chrono` + `chrono-tz`, `moka` (cache), `tracing`, `thiserror`.

## 5. Configuration

Everything through environment variables, with sensible defaults.

| Variable | Default | Purpose |
|----------|---------|---------|
| `BIND_ADDR` | `0.0.0.0:8080` | listen address |
| `DATA_DIR` | `/data` | SQLite database directory |
| `UPSTREAM_URL` | muoversiatorino URL | GraphQL endpoint |
| `UPSTREAM_TIMEOUT_SECS` | `10` | upstream call timeout |
| `DEPARTURES_CACHE_SECS` | `20` | departures cache lifetime |
| `STOPS_SYNC_TIME` | `04:00` | nightly sync time (Europe/Rome) |
| `DEFAULT_FEEDS` | `gtt` | feeds used in search when not specified |
| `API_TOKEN` | empty | when set, required as `Authorization: Bearer` |
| `RUST_LOG` | `info` | log level |

## 6. Upstream client

- A single shared `reqwest::Client` instance, with timeouts, an explicit `User-Agent` and gzip compression.
- Two queries:
  - `StopDepartures`: `stop(id)` with details and `stoptimesWithoutPatterns` (number of departures and time window as parameters);
  - `AllStops`: full list with coordinates, vehicle mode and lines.
- GraphQL variables are always passed as `variables`, never concatenated into the query string.
- Distinct errors: upstream unreachable, timeout, non-2xx HTTP, response with `errors`, unknown stop (`stop: null`).
- A single retry, with a short delay, for network errors; no retry for invalid queries.

## 7. Times

A single function in `time.rs`:

```
time = serviceDay (Unix) + seconds  →  DateTime<Europe/Rome>
```

- Handles seconds above 86,400 without special cases, since it starts from the absolute timestamp.
- Delay = `realtimeDeparture - scheduledDeparture`, in seconds, possibly negative (early).
- Dedicated tests: trip after midnight, last Sunday of March, last Sunday of October.

## 8. Stop index

### Schema

```sql
CREATE TABLE stops (
  id          TEXT PRIMARY KEY,   -- gtfsId, e.g. "gtt:39"
  feed        TEXT NOT NULL,      -- "gtt", "2", "3", "4"
  code        TEXT NOT NULL,      -- code shown to the user
  name        TEXT NOT NULL,      -- original name
  description TEXT,
  lat         REAL NOT NULL,
  lon         REAL NOT NULL,
  mode        TEXT,               -- BUS, TRAM, SUBWAY, RAIL, FUNICULAR
  lines       TEXT NOT NULL       -- JSON: [{"id": "...", "name": "4", "mode": "TRAM"}]
);
CREATE INDEX stops_code ON stops(code);
CREATE INDEX stops_latlon ON stops(lat, lon);

-- normalized text (name + description + expanded abbreviations)
CREATE VIRTUAL TABLE stops_fts USING fts5(id UNINDEXED, text, tokenize = 'trigram');

CREATE TABLE sync_state (
  key   TEXT PRIMARY KEY,         -- e.g. "stops"
  at    TEXT NOT NULL,            -- last successful sync
  count INTEGER NOT NULL
);
```

### Sync
- On startup if the index is empty or older than 24 hours, then every night at `STOPS_SYNC_TIME`.
- Full replacement within a single transaction: searches always see either the old or the new data, never a half-written index.
- If the sync fails, the old data stays and it retries with backoff (5 minutes, 15, 1 hour).

### Name normalization
- Lowercase and accent removal.
- Abbreviation expansion: `c.so` → `corso`, `v.` → `via`, `p.za` / `p.zza` → `piazza`, `l.go` → `largo`, `staz.` → `stazione`, `str.` → `strada`. The list is completed by looking at real names.
- The same normalization is applied to the search text.

### Search
1. If the search text is a code, an exact code match comes first.
2. Full-text search with trigrams (also finds partial words, e.g. `nuov`).
3. For texts shorter than 3 characters, `LIKE` is used as a fallback.
4. Ordering: exact code, then FTS relevance, then feed (`gtt` before the others).

### Nearby stops
Computed locally, without calling the upstream: first a bounding-box filter on lat/lon (uses the index), then the exact distance (haversine) for ordering.

## 9. Departures

- Upstream call with the requested time window and number of departures.
- In-memory cache per stop (`moka`, 20 seconds). Concurrent requests for the same stop result in a single upstream call.
- Filters (lines, maximum count) are applied to the cached response, so changing a filter does not trigger new calls.
- If the upstream fails and the cache holds a response less than 2 minutes old, it is returned with `"stale": true`; otherwise an error.

## 10. Favorites

```sql
CREATE TABLE favorites (
  id         INTEGER PRIMARY KEY,
  stop_id    TEXT NOT NULL,       -- gtfsId; no foreign key, the stop may disappear after a sync
  label      TEXT,                -- e.g. "Home → work"
  lines      TEXT,                -- JSON, optional line filter
  position   INTEGER NOT NULL,    -- manual ordering
  created_at TEXT NOT NULL
);
```

If a favorite stop no longer exists in the index, the response flags it with `"missing": true` instead of hiding it.

## 11. API

All responses are JSON. Times are in ISO 8601 format with time zone.

In paths, `{stop}` accepts the GTT code (`39`) or the full `gtfsId` (`gtt:39`, `2:S00219`).

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/stops?q=&feeds=&limit=` | search by name or code |
| GET | `/api/stops/nearby?lat=&lon=&radius=&limit=` | nearby stops (default radius 300 m) |
| GET | `/api/stops/{stop}` | stop details with lines |
| GET | `/api/stops/{stop}/departures?lines=&minutes=&limit=` | next departures (`minutes` 1–120, default 60; `limit` 1–100, default 20) |
| GET | `/api/favorites` | list favorites |
| POST | `/api/favorites` | add a favorite |
| PATCH | `/api/favorites/{id}` | change label, lines or position |
| DELETE | `/api/favorites/{id}` | delete a favorite |
| GET | `/health` | the process is alive (for Docker) |
| GET | `/api/status` | upstream status, last sync, stop count |

### Example: departures

```json
{
  "stop": { "id": "gtt:39", "code": "39", "name": "PORTA NUOVA" },
  "generated_at": "2026-10-29T16:12:05+01:00",
  "stale": false,
  "departures": [
    {
      "line": "67",
      "mode": "BUS",
      "headsign": "MONCALIERI, TESTONA - PIAZZA MARCONI",
      "scheduled": "2026-10-29T15:59:00+01:00",
      "expected": "2026-10-29T16:11:29+01:00",
      "delay_seconds": 749,
      "realtime": true,
      "canceled": false,
      "trip_id": "gtt:28459640U",
      "minutes": 0
    }
  ]
}
```

### Error format

```json
{ "error": { "code": "stop_not_found", "message": "Stop 99999 not found" } }
```

| Case | HTTP | `code` |
|------|------|--------|
| Invalid parameters | 400 | `invalid_request` |
| Missing or wrong token | 401 | `unauthorized` |
| Unknown stop | 404 | `stop_not_found` |
| Upstream error | 502 | `upstream_error` |
| Upstream timeout | 504 | `upstream_timeout` |

## 12. Docker

- **Multi-stage Dockerfile**:
  - build with `rust:1.95-slim-bookworm` and `cargo-chef`, so dependencies stay cached between builds;
  - runtime `debian:bookworm-slim` with only `ca-certificates` and `tzdata`;
  - non-root user.
- **Health check** without curl in the image: the binary itself has a `healthcheck` subcommand that calls `/health`.
- **docker-compose.yml**: port 8080, volume `./data:/data`, `restart: unless-stopped`, `TZ=Europe/Rome`, memory limit.
- Target: image under 40 MB.

## 13. Tests

- **Unit**: time conversion (midnight, daylight saving), name normalization, search ordering.
- **With fixtures**: real upstream responses saved in `tests/fixtures/` and served by a fake server (`wiremock`). Cases covered: normal response, unknown stop, HTTP 500, timeout.
- **API**: endpoint tests against a temporary SQLite database.
- **Manual smoke test**: a script that calls the real upstream, to notice when something changes. Not part of the automated tests.
- **Code quality**: `cargo fmt`, `cargo clippy` and `cargo test` in GitHub Actions on every push.

## 14. Development order

Each step ends with something verifiable and is developed in its own branch with a PR to `main`. When all steps are complete, `v0.1.0` is released.

1. **Skeleton**: crate, configuration, logging, `/health`, CI. Check: `curl /health` responds.
2. **Departures**: upstream client, time conversion, `departures` endpoint with cache. Check: manual comparison with muoversiatorino.it for stop 39.
3. **Stop index**: migrations, sync, normalization, search, nearby stops, details. Check: the search criteria in section 1.
4. **Favorites**: table and endpoints.
5. **Robustness**: `stale` responses, retry, `/api/status`, optional token.
6. **Docker and deployment**: arm64 image, compose, start on the Orange Pi. Check: all criteria in section 1 on the Pi.

## 15. Out of scope for this phase

- Rules, scheduler and notifications (Phase 2)
- Delay history (to be decided in Phase 2)
- Service alerts (`alerts`): the upstream always returns an empty list, to be re-checked later
- Web interface (Phase 3)
- Multiple users

## 16. Risks

| Risk | Mitigation |
|------|------------|
| The upstream changes schema or stops responding | Clear errors, smoke test, search independent of the upstream; automatic alert in Phase 2 |
| The upstream starts rate limiting | Cache, a single call per stop, sync once a day |
| Incomplete stop data (e.g. metro without code) | Fallback code from the id, tests on known cases |
