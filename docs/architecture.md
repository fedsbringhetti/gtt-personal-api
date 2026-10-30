# Architecture

What the service is, how its parts fit together, and where things live in the code. The step-by-step story of how each part was built is in [development/](development/README.md).

## What it is

A small HTTP service for real-time public transport departures in Turin. It reads data from the OpenTripPlanner GraphQL API behind muoversiatorino.it (unofficial) and adds what that API lacks or makes awkward:

- **stop search by name** (the upstream name search is broken) and by position, on a local copy of all ~19,400 stops;
- **departures** with real-time times, delays and line filters, cached so the upstream is called as little as possible;
- **favorite stops** with labels, line filters and a manual order;
- **resilience**: retries, stale data when the upstream fails, a status endpoint, optional token.

It is a single Rust binary with an SQLite database, built as a Docker image for the Orange Pi 5 (ARM64).

## How it works

### Components

```
              HTTP (axum)
                  │
  ┌───────────────┼───────────────────────────┐
  │ api/          │ auth (optional token)      │
  │  stops ──────►│ departures ──► cache ──┐   │
  │  stops ──────►│ stop index (SQLite) ◄─┐│   │
  │  favorites ──►│ favorites (SQLite)    ││   │
  │  status ─────►│ upstream health ◄─────┼┤   │
  └───────────────┴───────────────────────┼┼───┘
                                          ││
         nightly sync task ───────────────┘│
                                           ▼
                                upstream client ──► OpenTripPlanner GraphQL
                                (retry, health)      (muoversiatorino.it)
```

`main.rs` reads the configuration, opens the database (applying migrations), starts the background sync task and serves HTTP until Ctrl+C or SIGTERM, finishing requests in progress.

### Departures

1. The handler validates the parameters and turns `39` into `gtt:39`.
2. `DeparturesService` looks in the cache. On a miss it asks the upstream for the next 2 hours (up to 200 departures) in a single GraphQL query. Concurrent requests for the same stop share that call.
3. Each departure's time is `serviceDay + seconds`, converted to Europe/Rome. `serviceDay` is "noon minus 12 hours" of the service date (the GTFS convention), so times stay right after midnight and on daylight saving days.
4. The result is cached for `DEPARTURES_CACHE_SECS`, and also kept for 2 minutes as "last good" data.
5. Filters (lines, window, limit) are applied to the cached data, so changing them does not call the upstream again.
6. If the upstream fails and "last good" data exists, it is returned with `stale: true`.

### Stop index

- A background task downloads all stops (one query, about 5 MB) on startup when the index is missing or older than 24 hours, then every night at `STOPS_SYNC_TIME`. On failure it retries after 5 minutes, 15 minutes, then every hour.
- Each sync replaces the whole index in one transaction: a search sees either the old or the new data. An empty upstream response is rejected and the old index kept.
- For each stop the index stores the original name, a normalized name and a normalized search text (name + description), and an FTS5 trigram index on the search text.
- **Normalization**: lowercase, accents removed, punctuation removed, abbreviations expanded (`c.so` → `corso`, `v.` → `via`, `bv.` → `bivio`, `m.to` → `monferrato`, ...; the list comes from the most frequent abbreviations in real names), saint variants unified (`santa`, `sant'`, `s.` → `san`). The same normalization is applied to queries.
- **Matching**: words of 3 or more letters use the trigram index (partial words match); shorter words must start a word; a code or id matches directly.
- **Nearby**: a bounding box on an indexed lat/lon narrows the candidates, then the exact distance (haversine) filters and sorts them.
- The stop code shown is the upstream `code`, or the part of the id after `:` when missing (GTT metro stops).

### Favorites

Stored in SQLite with a 0-based `position` kept contiguous: inserts go at the end, moves shift the others, deletes close the gap, each in a transaction. A favorite stores only the stop id; its details are read from the index at every request, so a stop removed by a sync shows up as `missing` instead of disappearing.

### Robustness

- **Retry**: one retry after 300 ms on connection errors and HTTP 502–504. Not on timeouts (they already took the whole timeout), GraphQL errors or invalid responses (retrying would give the same result). GraphQL errors arrive with HTTP 500, so the body is checked before the status.
- **Upstream health**: every upstream call, including syncs, records its outcome; `/api/status` shows it. A GraphQL validation error usually means the upstream schema changed and the queries need fixing.
- **Token**: compared in constant time; never logged.

### Data

One SQLite file in `DATA_DIR` (`gtt.db`, plus `gtt.db-wal` and `gtt.db-shm` in WAL mode). Migrations in `migrations/` are compiled into the binary and applied at startup. Tables: `stops`, `stops_fts`, `sync_state`, `favorites`.

## Code map

| File | Responsibility |
|------|----------------|
| `src/main.rs` | startup, wiring, graceful shutdown, `healthcheck` subcommand |
| `src/config.rs` | environment variables → `Config`, with validation |
| `src/time.rs` | `serviceDay` + seconds → local time; `Clock` trait (real or fixed for tests) |
| `src/db.rs` | SQLite pool and migrations |
| `src/upstream/mod.rs` | GraphQL client: queries, errors, retry |
| `src/upstream/queries.rs`, `types.rs` | query texts and response shapes |
| `src/upstream/health.rs` | outcome of recent upstream calls |
| `src/departures.rs` | departures: conversion, sorting, filtering, cache, stale fallback |
| `src/stops/mod.rs` | stop index storage, `Feeds`, line sorting |
| `src/stops/normalize.rs` | name normalization and abbreviations |
| `src/stops/search.rs` | search ranking and nearby stops |
| `src/stops/sync.rs` | nightly sync scheduling and retries |
| `src/favorites.rs` | favorites storage and ordering |
| `src/api/mod.rs` | router, shared `AppState`, token layer |
| `src/api/stops.rs` | search, nearby, details and departures handlers |
| `src/api/favorites.rs` | favorites handlers and input cleaning |
| `src/api/status.rs`, `health.rs` | `/api/status`, `/health` |
| `src/api/auth.rs` | `API_TOKEN` middleware |
| `src/api/error.rs` | error codes and JSON error responses |

Reading order for a first look: `main.rs` → `api/mod.rs` → `api/stops.rs` → `departures.rs` → `upstream/mod.rs` → `stops/` → `favorites.rs`.

## Known limitations

- **Unofficial upstream**: it can change or disappear. `/api/status` and the logs show failures; a schema change shows up as a GraphQL validation error.
- **No vehicle positions** in the upstream API. A possible source (the 5T MQTT broker) is noted under "Future ideas" in [PLAN.md](../PLAN.md).
- **Search**: stops whose name lacks the searched words but whose description has them rank last (e.g. `pza castello` lists towns' "P.ZA CASTELLO" stops before Turin's "CASTELLO").
- **Stale data** covers only 2 minutes of upstream failure; after that, departures return an error.
- **Release builds on the Mac** fail (the linker produces a malformed `sqlx-macros` library); debug builds and Docker builds work.
- **Single user**: no accounts; favorites are shared by whoever has the token.

Next: Phase 2, the web app and the first deployment ([plan/phase-2.md](plan/phase-2.md)); then Phase 3, rules, trip tracking and Web Push notifications ([plan/phase-3.md](plan/phase-3.md)).
