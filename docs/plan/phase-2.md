# Phase 2 — Web app: detailed plan

Status: **approved** on 2026-10-30. In development since 2026-10-30. On 2026-11-02 authorization (decisions S and T) and the first deployment moved to [Phase 4](phase-4.md): the whole service is built and tested locally first.

Builds on Phase 1: the API (stops, departures, favorites, status), the Docker image and the deployment files.

## 1. Goal

A simple, modern interface on the phone, over the existing API, tested locally:
- search a stop by name or near me, see its departures update by themselves, keep favorites;
- installable on iPhone and Android, in Italian.

It also lays the ground for Phase 3, whose notifications (Web Push) need an installable web app with a service worker.

### Definition of done

Checked on the Mac (browser at phone size and iOS Simulator), where `localhost` allows location and the service worker; real phones are checked in Phase 4 (decision 12 in `PLAN.md`).

- [ ] The web app finds a stop by name and by position, and shows its departures refreshing automatically.
- [ ] The home screen shows the favorites with their next departures; favorites can be added, renamed, filtered by line, reordered and removed.
- [ ] It installs on the Home Screen (iOS Simulator) and opens full screen, with its own icon.
- [ ] Without network it still opens and shows the last loaded data, marked as old.
- [ ] It runs in the local container, survives a restart, idle memory under 60 MB.
- [ ] The Docker image stays under 50 MB to download.

## 2. Decisions

| # | Topic | Decision |
|---|-------|----------|
| — | Order, phones, language | From `PLAN.md` (decisions 7, 8, 9, 12): web app before rules; iPhone and Android, designed for iPhone's stricter rules; Italian; real phones checked at deployment, in Phase 4 |
| — | Serving | The same binary serves the web app and the API: one container, one origin |
| N | Frontend technology | **Svelte** with **Vite**, compiled to static files that the Rust binary embeds. TypeScript, so API responses have types. Plain Svelte, not SvelteKit: no server rendering is needed and a small router of our own covers six screens (decided 2026-10-30). **Node 24 LTS** everywhere: local, Docker, CI |
| O | Look and feel | Clean, iOS-like (system font, generous spacing, rounded cards, light/dark following the phone), with a **transport character**: line badges colored by vehicle type, departure-board rows, loading indicators with a moving bus or tram. To be refined during development |
| P | Screens | Home, search, stop, near me (list, no map), favorites, info (section 4) |
| Q | Position | Asked only when tapping "vicino a me"; used for that search only, never stored |
| R | Offline | The app shell opens without network; the last loaded favorites and departures stay visible with "dati delle HH:MM, sei offline"; no offline search |
| S, T | Authorization, hostname and Access policy | Moved to [Phase 4](phase-4.md) on 2026-11-02. Until then the web app is tested with `API_TOKEN` unset: it stores no token, so it cannot call an API that requires one |

## 3. Architecture

```
browser ──▶ :8080 (cargo run) or :8100 (container)
                  │
                  ├─ /api/*  → API (open, or API_TOKEN if set; Access JWT added in Phase 4)
                  ├─ /health → open
                  └─ /*      → embedded web app (index.html for unknown paths)
```

- **`web/`**: the Svelte project (Vite, TypeScript). `npm run build` writes static files to `web/dist/`.
- **Embedding**: the binary includes `web/dist/` at compile time (`rust-embed`), so the image still has a single file to run. In debug builds the files are read from disk, so a rebuild of the frontend does not need a Rust rebuild. A missing `web/dist/` does not break the Rust build: `cargo build` and `cargo test` work without Node, and the server logs a warning at startup.
- **Routing**: the app uses normal paths (`/fermata/39`); the backend answers any path that is not `/api/*`, `/health` or a file with `index.html`, and the app shows the right screen. Unknown `/api/*` paths and missing files under `/assets/` stay a real 404, so the browser never receives HTML in place of a script.
- **Caching headers**: files with a content hash in the name are cached forever; `index.html`, the manifest and the service worker are never cached, so updates arrive immediately.
- **Development**: `npm run dev` (Vite on port 5173, forwarding `/api` to `cargo run` on 8080) for instant reloads while working on the interface.

## 4. Screens (decision P)

All text in Italian. Departures refresh every 30 seconds while the screen is visible and stop when the app is in the background.

| Screen | Path | Content | API |
|--------|------|---------|-----|
| Home | `/` | search bar, "vicino a me" button, favorites as cards with the next 2–3 departures each | `/api/favorites/departures` (new, section 5) |
| Search | `/cerca?q=` | results while typing (after 250 ms of pause): name, code, lines | `/api/stops?q=` |
| Stop | `/fermata/{stop}` | name, code, lines as filter chips, star to save it, live departures: line badge, destination, minutes (large), expected time, delay in color, real-time indicator, "in arrivo" at 0 minutes | `/api/stops/{stop}`, `/api/stops/{stop}/departures` |
| Near me | `/vicino` | after the location permission: stops by distance, with lines | `/api/stops/nearby` |
| Favorites | `/preferiti` | rename, choose lines, move up/down, remove | `/api/favorites` |
| Info | `/info` | version, upstream status, last stop update, how to install on the Home Screen | `/api/status` |

Phase 3 will add: "avvisami" on a departure, the live tracking page, rules, notification settings.

## 5. Backend changes

- **Static files and fallback** (section 3).
- **`GET /api/favorites/departures?limit=3`**: the favorites with their next departures in one call, so the home screen makes one request instead of one per favorite. Uses the departures cache; a favorite whose departures fail is returned with an error flag instead of failing the whole response.

## 6. Look and feel (decision O)

- **Design tokens** as CSS variables: colors, spacing, radius, font sizes; one set for light, one for dark, chosen by the phone's setting.
- **Line badges**: colored by vehicle type (tram, bus, metro, rail), the line name in bold.
- **Departure rows** in the style of a departure board: line, destination, minutes aligned and large.
- **Loading**: a small bus or tram moving along a dotted line, instead of a generic spinner; skeleton rows while departures load.
- **Touch**: targets of at least 44 px, safe areas of the iPhone respected, no zoom on inputs.
- Kept deliberately simple so it can be changed later; the first screens are reviewed together before moving on.

## 7. Installable app (PWA) and offline (decision R)

- **Manifest**: name, short name, icons (192, 512, maskable), `display: standalone`, theme and background colors.
- **iPhone**: `apple-touch-icon`, `apple-mobile-web-app-capable`, status bar style, launch background color.
- **Service worker** (with `vite-plugin-pwa`, "inject manifest" mode so Phase 3 can add push handling): caches the app shell; API calls always go to the network.
- **Last data**: the app keeps the last favorites and departures it loaded (in the browser's storage) and shows them, marked with their time, when the network is missing.
- **Updates**: when a new version is deployed, the app shows "nuova versione disponibile" and reloads on tap.

## 8. Docker and CI

- **Dockerfile**: a new first stage on `node:24-slim` builds `web/dist/`; the Rust builder copies it before `cargo build`. The runtime image does not change.
- **CI**: a second job installs Node, runs `npm ci`, the Svelte type check, the frontend tests and the build. The two jobs run in parallel and independently: the Rust tests for static files use a small fixture folder, not the real build, and the Docker build is where the two meet (decided 2026-10-30).
- **`.dockerignore`**: excludes `web/node_modules` and local builds.

## 9. Tests

- **Rust**: static files (fallback to `index.html`, cache headers); `/api/favorites/departures`.
- **Frontend**: unit tests (Vitest) for formatting (minutes, delays, times) and the API client; Svelte type check.
- **By hand, on the Mac**: each screen in the browser at phone size and in the iOS Simulator, installed and in the browser; offline; dark mode. Real phones: Phase 4.

## 10. Development order

Each step in its own branch with a PR to `main` and its chapter in `docs/development/`. When all steps are complete, `v0.2.0` is released (Docker image published, not deployed).

1. **Frontend skeleton**: `web/` with Svelte, Vite and TypeScript; embedding and fallback in the binary; Node stage in the Dockerfile; CI job. Check: a placeholder page served by `cargo run` and by the container.
2. **Design base**: tokens, light/dark, layout and navigation, line badges, transport loaders. Reviewed together before going on.
3. **Search and stop**: search screen, stop screen with live departures and line filters.
4. **Home and favorites**: `/api/favorites/departures`, home cards, favorites management, star on the stop screen.
5. **Near me**: location permission and nearby list.
6. **PWA and offline**: manifest, icons, service worker, last data, update prompt, iPhone details.
7. **Info screen**.
8. **Release** `v0.2.0`.

## 11. Out of scope for this phase

- Rules, trip tracking, notifications (Phase 3)
- Authorization with Cloudflare Access and deployment (Phase 4)
- Map of stops or vehicles (future idea)
- Multiple users
- English interface

## 12. Risks

| Risk | Mitigation |
|------|------------|
| iPhone PWA quirks (storage limits, install flow, standalone behavior) | Tested in the iOS Simulator from step 6, on a real iPhone in Phase 4; Info screen explains how to install |
| Node build makes Docker builds slower | Separate cached stage; only rebuilt when `web/` changes |
| The frontend grows heavy on the phone | Svelte compiles to small files; size checked at each step |
