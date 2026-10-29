# Development plan — gtt-personal-api

Personal service for real-time GTT public transport departures in Turin, hosted in Docker on an Orange Pi 5 (ARM64).
Inspired by [madbob/gtt-pirate-api](https://github.com/madbob/gtt-pirate-api).

This document defines the phases. The detailed plan for each phase is written together, one phase at a time, before development starts.

## Context

- **Data source**: OpenTripPlanner GraphQL API at `https://plan.muoversiatorino.it/otp/routers/mato/index/graphql` (unofficial, may break without notice).
- **What works upstream** (verified on 2026-10-29):
  - `stop(id: "gtt:<code>")` returns stop details and departures in a single query.
  - `stopsByRadius(lat, lon, radius)` for nearby stops.
  - `stops { ... }` returns the full list (about 7,058 GTT stops).
  - Available feeds: `gtt` (GTT urban, suburban, SFM rail, tourist), `2` (Trenitalia), `3` (Arriva), `4` (regional bus companies).
- **What does not work upstream**: `stops(name: ...)` returns an empty list, so name search must use a local index.
- **Useful data available**: line, destination (`headsign`), vehicle mode (bus/tram/metro), scheduled and real-time times, `serviceDay`, trip id, coordinates, lines serving a stop.

## Stack

- Rust: axum, tokio, reqwest, sqlx + SQLite (FTS5), serde, chrono-tz
- Multi-stage Docker image for `linux/arm64`, SQLite database on a volume

## Cross-cutting decisions

| # | Decision | Options | Choice |
|---|----------|---------|--------|
| 1 | Transport feeds | GTT only / all feeds | All indexed, search defaults to `gtt` only |
| 2 | Notification channel | Telegram / ntfy / Web Push (PWA) | _to decide (Phase 2)_ |
| 3 | Remote access | LAN only / Tailscale / Cloudflare Tunnel + login | _to decide_; optional `API_TOKEN` in the meantime |
| 4 | Users | just me / multiple people | Just me |
| 5 | Rule types | scheduled summary / trip tracking / "leave now" | _to decide (Phase 2)_ |
| 6 | Delay history | yes / no | Deferred to Phase 2 |

## Workflow

- **Language**: everything in the repository is in English: file and directory names, document contents, code, identifiers, branch names, commit messages, PR titles and descriptions.
- **Branches**: `main` always works. Every change starts from `main` in a short-lived branch (`feat/...`, `fix/...`, `docs/...`) and merges back through a PR once CI (`cargo fmt`, `cargo clippy`, `cargo test`) passes. No `dev` branch.
- **Releases**: semantic version tags on `main` (`v0.1.0` at the end of Phase 1, `v0.2.0` at the end of Phase 2, ...). To try a version on the Pi before releasing it, use a pre-release tag (`v0.2.0-rc1`).
- **Docker images**: built on the Mac for `linux/arm64` and pushed to `ghcr.io/fedsbringhetti/gtt-personal-api`, tagged with the version and `latest`.
- **Deployment**: on the Orange Pi the compose file pins an exact version (e.g. `:0.1.0`), never `latest`. Rolling back means changing the tag.

---

## Phase 1 — Core backend

**Goal**: a reliable API to search stops and read departures, already running on the Orange Pi.

**Features**
- Client for the upstream GraphQL API (a single query per stop)
- Local copy of all stops, refreshed nightly, with name search
- Nearby stops search
- Stop details with the lines serving it
- Stop departures: line, destination, scheduled and real-time times, delay, vehicle mode, line filter
- Short in-memory cache to avoid overloading the upstream
- Favorite stops
- Health check, including upstream reachability
- Dockerfile and `docker-compose.yml` for arm64

**Detailed plan**: [docs/phase-1.md](docs/phase-1.md) (approved)

---

## Phase 2 — Rules and notifications

**Goal**: automatically receive the departures I need, when I need them.

**Features**
- Scheduled summary (e.g. "Mon–Fri at 7:40, lines 4 and 15 from stop X")
- Trip tracking (e.g. "the 8:05 bus: notify me 5 minutes before or if it is late")
- Possibly "leave now", based on walking time to the stop
- Send a test notification for a rule
- Alert myself when the upstream stops working

**Detailed plan**: [docs/phase-2.md](docs/phase-2.md) (draft)

---

## Phase 3 — Frontend

**Goal**: a modern, simple interface, usable from a phone.

**Features**
- PWA installable on the phone, served by the same binary as the backend
- Stop search by name and by location
- Departures view with automatic refresh
- Management of favorites and rules

**Detailed plan**: _to be written_

---

## Future ideas

Not planned in any phase yet.

### Real-time vehicle positions

Found on 2026-10-29 by inspecting [InfomezziMap](https://infomezzi.marmox.to.it/immap/).

- **Source**: the 5T MQTT broker at `wss://mapi.5t.torino.it/scre`, the same one used by muoversiatorino.it.
- **Topics**: subscribe to `/<line>/#` (or `#` for all lines); messages arrive on `/<line>/<vehicleId>`.
- **Payload**: JSON array `[lat, lon, heading, ?, tripId, direction, nextStop, ...]`; a longer array marks a full vehicle.
- **Possible uses**: a vehicle map in the frontend; "your bus is N stops away" during trip tracking; earlier detection of runs that never started.
- **Open question**: the site connects with `Origin: https://www.muoversiatorino.it`. Using the broker from this service would mean presenting the same origin. To decide before any work starts.
- **Not verified**: whether the broker accepts other origins, and whether `tripId` matches the OpenTripPlanner `trip.gtfsId`.
- **Not to use**: InfomezziMap's own `getWsData.php` endpoint; it is a private server with its own token.
