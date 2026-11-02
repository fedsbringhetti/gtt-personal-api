# API reference

How to run the service and use every endpoint. For settings see [configuration.md](configuration.md); for the internals see [../architecture.md](../architecture.md).

## Running it

| Where | How |
|-------|-----|
| Locally, from source | `cargo run`, then `http://localhost:8080`. Data goes to `./data`. The web app is there too once built (`npm ci && npm run build` in `web/`) |
| Locally, with Docker | build the image (`scripts/build-image.sh`), `cp .env.example .env`, `docker compose up -d`, then `http://localhost:8100` |
| Home server | not deployed yet, planned in Phase 4; steps in [deploy.md](deploy.md) |

On first start the service downloads all stops (2–3 seconds). Until then the stop endpoints answer `503 index_not_ready`.

Paths outside `/api/*` and `/health` serve the web app. All API responses are JSON; times are ISO 8601 with time zone (`2026-10-29T17:23:53+01:00`).

## Authentication

If `API_TOKEN` is set, every `/api/*` request needs the header `Authorization: Bearer <token>`. `/health` never does. The examples below assume:

```bash
export BASE=http://localhost:8100
export AUTH="Authorization: Bearer <your token>"
```

## Typical flow

1. Find the stop code: by name (`/api/stops?q=`) or near you (`/api/stops/nearby`).
2. Read its departures (`/api/stops/{stop}/departures`), optionally filtered by line.
3. Save it as a favorite to find it again quickly.

## Find a stop by name or code

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

## Find stops near a point

```bash
curl -s -H "$AUTH" "$BASE/api/stops/nearby?lat=45.0712&lon=7.6853&radius=200"
```

| Parameter | Default | Notes |
|-----------|---------|-------|
| `lat`, `lon` | required | decimal degrees |
| `radius` | 300 | meters, 1–2000 |
| `feeds`, `limit` | as above | |

Same stop objects, nearest first, each with `"distance_m"`.

## Stop details

```bash
curl -s -H "$AUTH" "$BASE/api/stops/39"
```

`{stop}` is a GTT code (`39`) or a full id (`gtt:39`, `2:S00219`). Returns one stop object, or `404 stop_not_found`.

## Departures

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

## Favorites

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

```bash
curl -s -H "$AUTH" "$BASE/api/favorites/departures?limit=3"
```

Every favorite, in order, with its next departures: what the web app's home screen shows, in one call.

| Parameter | Default | Notes |
|-----------|---------|-------|
| `limit` | 3 | departures per favorite, 1–10 |
| `minutes` | 60 | how far ahead, 1–120 |

```json
{
  "generated_at": "2026-10-29T17:23:53+01:00",
  "favorites": [{
    "id": 1, "stop_id": "gtt:39", "label": "Casa", "lines": ["67"], "position": 0,
    "created_at": "2026-10-29T16:23:53Z", "stop": {"id": "gtt:39", "code": "39", "name": "PORTA NUOVA"},
    "missing": false, "stale": false, "error": null,
    "departures": [{"line": "67", "minutes": 0, "expected": "2026-10-29T17:23:53+01:00"}]
  }]
}
```

- Each favorite is shown as in `GET /api/favorites` (`stop` abridged here) plus `departures` (same fields as the stop departures, filtered by the favorite's `lines`), `stale` and `error`.
- When one stop fails, that favorite has `departures: []` and `error: {"code", "message"}` (codes as in [Errors](#errors)); the others are unaffected and the response is still 200.
- Stops are read in parallel and through the same cache as `/api/stops/{stop}/departures`.

## Status and health

```bash
curl -s "$BASE/health"
```

`{"status": "ok", "version": "0.2.0"}` while the process runs. Used by Docker; never needs the token.

```bash
curl -s -H "$AUTH" "$BASE/api/status"
```

Version, current time, `upstream` (`ok`, last success, last failure and error, consecutive failures) and `stop_index` (`synced_at`, `count`). The first place to look when something seems wrong.

## Errors

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
