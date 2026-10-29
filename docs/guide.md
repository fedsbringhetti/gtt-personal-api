# Guide

What the service does, how to use it, and how the code works. State: end of Phase 1 (core backend), version 0.1.0.

- [1. What it is](#1-what-it-is)
- [2. Running it](#2-running-it)
- [3. Using the API](#3-using-the-api)
- [4. Configuration](#4-configuration)
- [5. How it works](#5-how-it-works)
- [6. Code map](#6-code-map)
- [7. Development](#7-development)
- [8. Known limitations](#8-known-limitations)

## 1. What it is

A small HTTP service for real-time public transport departures in Turin. It reads data from the OpenTripPlanner GraphQL API behind muoversiatorino.it (unofficial) and adds what that API lacks or makes awkward:

- **stop search by name** (the upstream name search is broken) and by position, on a local copy of all ~19,400 stops;
- **departures** with real-time times, delays and line filters, cached so the upstream is called as little as possible;
- **favorite stops** with labels, line filters and a manual order;
- **resilience**: retries, stale data when the upstream fails, a status endpoint, optional token.

It is a single Rust binary with an SQLite database, built as a Docker image for the Orange Pi 5 (ARM64).

## 2. Running it

| Where | How |
|-------|-----|
| Locally, from source | `cargo run`, then `http://localhost:8080`. Data goes to `./data` |
| Locally, with Docker | build the image (`scripts/build-image.sh`), `cp .env.example .env`, `docker compose up -d`, then `http://localhost:8100` |
| Home server | follow [deploy.md](deploy.md); then `http://192.168.1.13:8100` |

On first start the service downloads all stops (2–3 seconds). Until then the stop endpoints answer `503 index_not_ready`.

## 3. Using the API

All responses are JSON; times are ISO 8601 with time zone (`2026-10-29T17:23:53+01:00`).

### Authentication

If `API_TOKEN` is set, every `/api/*` request needs the header `Authorization: Bearer <token>`. `/health` never does. The examples below assume:

```bash
export BASE=http://localhost:8100
export AUTH="Authorization: Bearer <your token>"
```

### Typical flow

1. Find the stop code: by name (`/api/stops?q=`) or near you (`/api/stops/nearby`).
2. Read its departures (`/api/stops/{stop}/departures`), optionally filtered by line.
3. Save it as a favorite to find it again quickly.

### Find a stop by name or code

```bash
curl -s -H "$AUTH" "$BASE/api/stops?q=porta%20nuova&limit=5"
```

| Parameter | Default | Notes |
|-----------|---------|-------|
| `q` | required | name, part of a name, or stop code; 1–100 characters |
| `feeds` | `DEFAULT_FEEDS` (`gtt`) | `all`, or a list such as `gtt,2` (`2` Trenitalia, `3` Arriva, `4` regional buses) |
| `limit` | 20 | 1–100 |

Response: `{"stops": [stop, ...]}`, where each stop is:

```json
{
  "id": "gtt:39", "feed": "gtt", "code": "39", "name": "PORTA NUOVA",
  "description": "PORTA NUOVA - M1 - STAZIONE",
  "lat": 45.06256, "lon": 7.67898, "mode": "BUS",
  "lines": [{"id": "gtt:6U", "name": "6", "mode": "BUS"}, {"id": "gtt:9U", "name": "9", "mode": "TRAM"}, {"id": "gtt:9BU", "name": "9B", "mode": "BUS"}]
}
```

Search tips:
- abbreviations are understood both ways: `c.so vittorio` = `corso vittorio`, `p.za` = `piazza`, `s.anna` = `santa anna` = `sant'anna`;
- partial words work (`nuov`), and the order of words does not matter;
- a code (`39`) or full id (`gtt:39`) comes first;
- results are ordered: exact code, exact name, name starting with the text, name containing it, description only; then GTT first, shorter names first.

### Find stops near a point

```bash
curl -s -H "$AUTH" "$BASE/api/stops/nearby?lat=45.0712&lon=7.6853&radius=200"
```

| Parameter | Default | Notes |
|-----------|---------|-------|
| `lat`, `lon` | required | decimal degrees |
| `radius` | 300 | meters, 1–2000 |
| `feeds`, `limit` | as above | |

Same stop objects, nearest first, each with `"distance_m"`.

### Stop details

```bash
curl -s -H "$AUTH" "$BASE/api/stops/39"
```

`{stop}` is a GTT code (`39`) or a full id (`gtt:39`, `2:S00219`). Returns one stop object, or `404 stop_not_found`.

### Departures

```bash
curl -s -H "$AUTH" "$BASE/api/stops/39/departures?lines=4,15&minutes=30&limit=10"
```

| Parameter | Default | Notes |
|-----------|---------|-------|
| `lines` | all | comma-separated line names, case-insensitive (`9b` = `9B`) |
| `minutes` | 60 | how far ahead, 1–120 |
| `limit` | 20 | 1–100 |

```json
{
  "stop": {"id": "gtt:39", "code": "39", "name": "PORTA NUOVA"},
  "generated_at": "2026-10-29T17:23:23+01:00",
  "fetched_at": "2026-10-29T17:23:20+01:00",
  "stale": false,
  "departures": [{
    "line": "67", "mode": "BUS", "headsign": "MONCALIERI, TESTONA - PIAZZA MARCONI",
    "scheduled": "2026-10-29T17:24:00+01:00", "expected": "2026-10-29T17:23:53+01:00",
    "delay_seconds": -7, "realtime": true, "canceled": false,
    "trip_id": "gtt:28459721U", "minutes": 0
  }]
}
```

- `expected` is the time to rely on; it equals `scheduled` when `realtime` is `false`.
- `minutes` is rounded down and never negative.
- `stale: true` means the upstream is failing and this is the last good data (at most 2 minutes old, see `fetched_at`).

### Favorites

```bash
curl -s -H "$AUTH" "$BASE/api/favorites"
```

Lists favorites in order; each has `id`, `stop_id`, `label`, `lines`, `position`, `created_at`, the full `stop`, and `missing` (true if a sync removed the stop).

```bash
curl -s -H "$AUTH" -H "Content-Type: application/json" -X POST "$BASE/api/favorites" -d '{"stop": "39", "label": "Home → station", "lines": ["9B", "52"]}'
```

Adds a favorite at the end (201). The stop must exist. `label` (at most 100 characters) and `lines` (at most 20) are optional.

```bash
curl -s -H "$AUTH" -H "Content-Type: application/json" -X PATCH "$BASE/api/favorites/1" -d '{"position": 0, "label": null}'
```

Changes only the fields given: `label` (`null` clears it), `lines`, `position` (0-based; past the end moves it last).

```bash
curl -s -H "$AUTH" -X DELETE "$BASE/api/favorites/1"
```

Deletes it (204); the following favorites move up.

### Status and health

```bash
curl -s "$BASE/health"
```

`{"status": "ok", "version": "0.1.0"}` while the process runs. Used by Docker; never needs the token.

```bash
curl -s -H "$AUTH" "$BASE/api/status"
```

Version, current time, `upstream` (`ok`, last success, last failure and error, consecutive failures) and `stop_index` (`synced_at`, `count`). The first place to look when something seems wrong.

### Errors

Always `{"error": {"code": "...", "message": "..."}}`:

| HTTP | `code` | When |
|------|--------|------|
| 400 | `invalid_request` | bad parameter or body (the message says which) |
| 401 | `unauthorized` | token missing or wrong |
| 404 | `stop_not_found`, `favorite_not_found` | unknown stop or favorite |
| 500 | `internal_error` | database problem (details in the logs) |
| 502 | `upstream_error` | upstream error and no recent data to fall back on |
| 503 | `index_not_ready` | stop index not downloaded yet |
| 504 | `upstream_timeout` | upstream too slow |

## 4. Configuration

Environment variables, all optional. In Docker they go in `.env` (see `.env.example`).

| Variable | Default | Purpose |
|----------|---------|---------|
| `API_TOKEN` | unset | bearer token for `/api/*`; unset means open (a warning is logged) |
| `BIND_ADDR` | `0.0.0.0:8080` | listen address |
| `DATA_DIR` | `data` (`/data` in Docker) | SQLite database folder |
| `STOPS_SYNC_TIME` | `04:00` | nightly stop sync, Europe/Rome |
| `DEFAULT_FEEDS` | `gtt` | feeds searched when a request does not say |
| `UPSTREAM_URL` | muoversiatorino endpoint | upstream GraphQL API |
| `UPSTREAM_TIMEOUT_SECS` | `10` | timeout for departures calls (the full stop list has 60 s) |
| `DEPARTURES_CACHE_SECS` | `20` | how long departures are reused |
| `RUST_LOG` | `info` | log level, e.g. `debug` or `info,gtt_personal_api::upstream=debug` |

An invalid value stops the service at startup with a message naming the variable.

## 5. How it works

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

## 6. Code map

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

## 7. Development

### Checks

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

The same checks run in GitHub Actions on every PR and on `main`.

### Tests

- **Unit tests** sit at the bottom of each file (`#[cfg(test)]`): times, normalization, ranking helpers, config, sync scheduling, token parsing.
- **Integration tests** in `tests/` start the real router with an in-memory database, a fixed clock and a fake upstream (`wiremock`): `departures.rs`, `stops.rs`, `favorites.rs`, `status.rs`, `auth.rs`, `health.rs`. Shared helpers are in `tests/common/mod.rs` (`test_app`, `synced_app`, `get`, `send`, `request`).
- **Fixtures** in `tests/fixtures/` are real upstream responses: `stop_39.json` (departures at Porta Nuova) and `all_stops.json` (44 stops around Porta Nuova plus a few chosen for abbreviations and duplicate codes).

### Common changes

- **New endpoint**: handler in `src/api/`, route in `src/api/mod.rs` (it is behind the token automatically), new error codes in `src/api/error.rs`, integration test in `tests/`.
- **Database change**: add `migrations/000N_description.sql`; it is applied at the next startup. Never edit a migration that has already run.
- **New upstream field**: add it to the query in `src/upstream/queries.rs` and to `types.rs`; refresh the fixtures if tests need it.
- **New setting**: add it to `Config` with a default and a test, then to the README, this guide and `.env.example`.

### Workflow and release

- Work in a short branch from `main` (`feat/...`, `fix/...`, `docs/...`), open a PR, merge when CI passes. Everything in the repository is in English.
- Release: bump `version` in `Cargo.toml`, tag `vX.Y.Z` on `main`, run `scripts/build-image.sh --push`, then update the image tag on the server ([deploy.md](deploy.md)).

## 8. Known limitations

- **Unofficial upstream**: it can change or disappear. `/api/status` and the logs show failures; a schema change shows up as a GraphQL validation error.
- **No vehicle positions** in the upstream API. A possible source (the 5T MQTT broker) is noted under "Future ideas" in `PLAN.md`.
- **Search**: stops whose name lacks the searched words but whose description has them rank last (e.g. `pza castello` lists towns' "P.ZA CASTELLO" stops before Turin's "CASTELLO").
- **Stale data** covers only 2 minutes of upstream failure; after that, departures return an error.
- **Release builds on the Mac** fail (the linker produces a malformed `sqlx-macros` library); debug builds and Docker builds work.
- **Single user**: no accounts; favorites are shared by whoever has the token.

Phase 2 (rules and Telegram notifications) is planned in [phase-2.md](phase-2.md).
