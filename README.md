# gtt-personal-api

Personal service for real-time GTT public transport departures in Turin, meant to run in Docker on an Orange Pi 5.
Inspired by [madbob/gtt-pirate-api](https://github.com/madbob/gtt-pirate-api).

- [docs/guide.md](docs/guide.md): what the service does, how to use the API, how the code works.
- [docs/deploy.md](docs/deploy.md): Docker image and deployment on the home server.
- [PLAN.md](PLAN.md): phases and decisions.

## Running with Docker

For now the image is used for local testing (the home server deployment is planned with the user interface, Phase 3):

```bash
scripts/build-image.sh
cp .env.example .env
docker compose up -d
```

The service is then on `http://localhost:8100`. See [docs/deploy.md](docs/deploy.md) for publishing the image to Docker Hub and deploying it on the home server.

## Running locally

```bash
cargo run
```

```bash
curl http://localhost:8080/health
curl 'http://localhost:8080/api/stops?q=porta%20nuova'
curl 'http://localhost:8080/api/stops/39/departures?lines=4,15&minutes=30'
```

On first start the service downloads all stops (about 19,000, a few seconds) into `DATA_DIR`; until then stop endpoints answer `503 index_not_ready`.

## API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | liveness check (never requires the token) |
| GET | `/api/status` | upstream health, last stop sync, stop count |
| GET | `/api/stops?q=&feeds=&limit=` | search stops by name or code; `feeds` is `all` or a list such as `gtt,2` |
| GET | `/api/stops/nearby?lat=&lon=&radius=&feeds=&limit=` | stops near a point, nearest first; `radius` 1–2000 m (default 300) |
| GET | `/api/stops/{stop}` | stop details with the lines serving it |
| GET | `/api/favorites` | favorite stops in order, with stop details |
| POST | `/api/favorites` | add a favorite: `{"stop": "39", "label": "Home", "lines": ["4", "15"]}` |
| PATCH | `/api/favorites/{id}` | change `label` (`null` clears it), `lines` or `position` |
| DELETE | `/api/favorites/{id}` | delete a favorite |
| GET | `/api/stops/{stop}/departures?lines=&minutes=&limit=` | next departures; `{stop}` is a GTT code (`39`) or a full gtfsId (`gtt:39`); `minutes` 1–120 (default 60), `limit` 1–100 (default 20). If the upstream fails, the last good data (up to 2 minutes old) comes back with `"stale": true` |

When `API_TOKEN` is set, every `/api/*` request needs `Authorization: Bearer <token>` (`/health` stays open). Generate one with `openssl rand -hex 32`.

Errors use the format `{"error": {"code": "...", "message": "..."}}`. See [docs/phase-1.md](docs/phase-1.md) for details.

## Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `BIND_ADDR` | `0.0.0.0:8080` | listen address |
| `UPSTREAM_URL` | muoversiatorino GraphQL endpoint | upstream API |
| `UPSTREAM_TIMEOUT_SECS` | `10` | upstream call timeout |
| `DEPARTURES_CACHE_SECS` | `20` | departures cache lifetime |
| `DATA_DIR` | `data` | directory of the SQLite database |
| `STOPS_SYNC_TIME` | `04:00` | nightly stop index sync, Europe/Rome time |
| `DEFAULT_FEEDS` | `gtt` | feeds searched when a request does not say: `all` or a list such as `gtt,2` |
| `API_TOKEN` | empty | bearer token required by `/api/*` when set |
| `RUST_LOG` | `info` | log level |

## Checks

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
