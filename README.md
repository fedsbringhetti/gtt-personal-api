# gtt-personal-api

Personal service for real-time GTT public transport departures in Turin, meant to run in Docker on an Orange Pi 5.
Inspired by [madbob/gtt-pirate-api](https://github.com/madbob/gtt-pirate-api).

See [PLAN.md](PLAN.md) for phases and decisions.

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
| GET | `/health` | liveness check |
| GET | `/api/stops?q=&feeds=&limit=` | search stops by name or code; `feeds` is `all` or a list such as `gtt,2` |
| GET | `/api/stops/nearby?lat=&lon=&radius=&feeds=&limit=` | stops near a point, nearest first; `radius` 1–2000 m (default 300) |
| GET | `/api/stops/{stop}` | stop details with the lines serving it |
| GET | `/api/stops/{stop}/departures?lines=&minutes=&limit=` | next departures; `{stop}` is a GTT code (`39`) or a full gtfsId (`gtt:39`); `minutes` 1–120 (default 60), `limit` 1–100 (default 20) |

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
| `RUST_LOG` | `info` | log level |

## Checks

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
