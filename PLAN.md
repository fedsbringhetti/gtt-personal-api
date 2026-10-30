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
| 2 | Notification channel | Telegram / ntfy / Web Push (PWA) | Web Push from the web app (decided 2026-10-30): no third-party app; works on iPhone (web app added to the Home Screen, iOS 16.4+) and Android |
| 3 | Remote access | LAN only / Tailscale / Cloudflare Tunnel + login | The existing Cloudflare Tunnel → Nginx Proxy Manager on the server (HTTPS), protected by **Cloudflare Access** (decided 2026-10-30); `API_TOKEN` stays as a second layer |
| 4 | Users | just me / multiple people | Just me. Shared data (delay history) is stored per run, not per person, so more users can be added later without reshaping it |
| 5 | Rule types | scheduled summary / trip tracking / "leave now" | Scheduled summary and trip tracking, both on the fly and as recurring rules; no "leave now" (no walking time), reminders "N minutes before" instead (decided 2026-10-30) |
| 6 | Delay history | yes / no | Yes, for tracked runs only, stored per run (date, trip, stop) and shared by every rule (decided 2026-10-30) |
| 7 | Phase order | rules first / web app first | Web app first (decided 2026-10-30): Phase 2 is the web app and the first deployment, Phase 3 rules, tracking and notifications, which need the web app for Web Push |
| 8 | Phones | iPhone / Android | Both: the web app is designed for the stricter case (iPhone, installable PWA) |
| 9 | Language of user-facing text | Italian / English / both | Italian (web app, notifications); code and docs stay in English |

## Workflow

- **Language**: everything in the repository is in English: file and directory names, document contents, code, identifiers, branch names, commit messages, PR titles and descriptions.
- **Branches**: `main` always works. Every change starts from `main` in a short-lived branch (`feat/...`, `fix/...`, `docs/...`) and merges back through a PR once CI (`cargo fmt`, `cargo clippy`, `cargo test`) passes. No `dev` branch.
- **Releases**: semantic version tags on `main` (`v0.1.0` at the end of Phase 1, `v0.2.0` at the end of Phase 2, ...), each with a GitHub Release and notes. To try a version on the Pi before releasing it, use a pre-release tag (`v0.2.0-rc1`).
- **Docker images**: built on the Mac for `linux/arm64` and pushed to Docker Hub as `federicosbringhetti/gtt-personal-api` (public), tagged with the version and `latest`.
- **Deployment**: first deployment with the web app, in Phase 2 (decided on 2026-10-29, moved from Phase 3 to Phase 2 on 2026-10-30 when the phases were swapped). Until then the Docker image is only used for local testing. Then: on the home server in `/home/federico/gtt-personal-api/`, host port 8100, data in the `./data` bind mount. The compose file pins an exact version (e.g. `:0.1.0`), never `latest`; rolling back means changing the tag. Steps in [docs/user/deploy.md](docs/user/deploy.md).

---

## Phase 1 — Core backend

**Goal**: a reliable API to search stops and read departures, packaged as a Docker image for the Orange Pi (first deployment with the web app, Phase 2).

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

**Detailed plan**: [docs/plan/phase-1.md](docs/plan/phase-1.md) (done; development story in [docs/development/](docs/development/README.md))

---

## Phase 2 — Web app and first deployment

**Goal**: a modern, simple interface on the phone over the existing API, deployed on the home server and reachable from anywhere.

**Features**
- Installable web app (PWA) for iPhone and Android, in Italian, built with Svelte and served by the same binary as the backend
- Stop search by name and by location, departures with automatic refresh, favorites
- First deployment on the home server, behind the Cloudflare Tunnel and Nginx Proxy Manager, protected by Cloudflare Access

**Detailed plan**: [docs/plan/phase-2.md](docs/plan/phase-2.md) (approved)

---

## Phase 3 — Rules, trip tracking and notifications

**Goal**: automatically receive the departures I need, when I need them, as Web Push notifications from the web app.

**Features**
- Scheduled summary (e.g. "Mon–Fri at 7:40, lines 4 and 15 from stop X")
- Trip tracking, on the fly from the departures list or as a recurring rule: reminders N minutes before, delay and cancellation alerts, a live page
- Delay history of tracked runs
- Rule management in the web app
- Alert when the upstream stops working

**Detailed plan**: [docs/plan/phase-3.md](docs/plan/phase-3.md) (decisions confirmed; to be completed before the phase starts)

---

## Future ideas

Not planned in any phase yet.

### Custom dates for rules

Rules already skip Italian public holidays and 24 June. Personal dates (holidays, days off) could be added as a list of extra dates to skip, per rule or for all rules. Proposed on 2026-10-30.

### Walking time ("leave now")

A "leave now" alert needs the walking time to the stop: entered by hand, estimated from the phone's position and the stop's coordinates, or computed with OpenTripPlanner's walking routes. Set aside on 2026-10-30 in favor of reminders "N minutes before" chosen by the user.

### Multiple users

Separate accounts with their own favorites and rules. Cloudflare Access can already let several email addresses in, but they would share the same data today. The delay history is stored per run, so it would be shared by all users and more precise with more of them.

### Real-time vehicle positions

Found on 2026-10-29 by inspecting [InfomezziMap](https://infomezzi.marmox.to.it/immap/).

- **Source**: the 5T MQTT broker at `wss://mapi.5t.torino.it/scre`, the same one used by muoversiatorino.it.
- **Topics**: subscribe to `/<line>/#` (or `#` for all lines); messages arrive on `/<line>/<vehicleId>`.
- **Payload**: JSON array `[lat, lon, heading, ?, tripId, direction, nextStop, ...]`; a longer array marks a full vehicle.
- **Possible uses**: a vehicle map in the frontend; "your bus is N stops away" during trip tracking; earlier detection of runs that never started.
- **Open question**: the site connects with `Origin: https://www.muoversiatorino.it`. Using the broker from this service would mean presenting the same origin. To decide before any work starts.
- **Not verified**: whether the broker accepts other origins, and whether `tripId` matches the OpenTripPlanner `trip.gtfsId`.
- **Not to use**: InfomezziMap's own `getWsData.php` endpoint; it is a private server with its own token.
