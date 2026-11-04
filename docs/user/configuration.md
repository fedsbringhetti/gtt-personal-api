# Configuration

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
| `GTT_REALTIME_URL` | `https://percorsieorari.gtt.to.it/das_gtfsrt/` | folder of GTT's real-time feeds; empty disables GTT's predictions and alerts (OpenTripPlanner only) |
| `GTT_TRIP_UPDATES_CACHE_SECS` | `20` | how long GTT's trip updates are reused |
| `GTT_ALERTS_POLL_SECS` | `300` | how often GTT's alerts are read, at least 1 |
| `GTT_GTFS_URL` | `https://www.gtt.to.it/open_data/gtt_gtfs.zip` | GTT's static data, for its `stops.txt`: checked every night 10 minutes after `STOPS_SYNC_TIME`, downloaded only when it changed. Empty disables it: alerts are then linked to stops by their lines only |
| `RUST_LOG` | `info` | log level, e.g. `debug` or `info,gtt_personal_api::upstream=debug` |

An invalid value stops the service at startup with a message naming the variable.

In Docker, `BIND_ADDR`, `DATA_DIR` and `TZ` are set by the image or `docker-compose.yml`; the rest goes in `.env` ([deploy.md](deploy.md)).
