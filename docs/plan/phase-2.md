# Phase 2 — Web app and first deployment: detailed plan

Status: **approved** on 2026-10-30. Only the hostname and the Access policy (decision T) are left for the deployment step. In development since 2026-10-30.

Builds on Phase 1: the API (stops, departures, favorites, status), the Docker image and the deployment files.

## 1. Goal

A simple, modern interface on the phone, over the existing API, running on the home server and reachable from anywhere:
- search a stop by name or near me, see its departures update by themselves, keep favorites;
- installable on iPhone and Android, in Italian;
- deployed on the home server behind the existing Cloudflare Tunnel and Nginx Proxy Manager, protected by Cloudflare Access.

It also lays the ground for Phase 3, whose notifications (Web Push) need an installable web app with a service worker.

### Definition of done

- [ ] From the phone, the web app finds a stop by name and by position, and shows its departures refreshing automatically.
- [ ] The home screen shows the favorites with their next departures; favorites can be added, renamed, filtered by line, reordered and removed.
- [ ] It installs on the Home Screen of an iPhone and of an Android phone and opens full screen, with its own icon.
- [ ] Without network it still opens and shows the last loaded data, marked as old.
- [ ] From outside home it is reachable only after the Cloudflare Access login; requests reaching port 8100 without the Access signature or `API_TOKEN` are refused; `/health` still works for Docker.
- [ ] It runs on the Orange Pi (Phase 1 criterion carried over), survives a restart, idle memory under 60 MB.
- [ ] The Docker image stays under 50 MB to download.

## 2. Decisions

| # | Topic | Decision |
|---|-------|----------|
| — | Order, phones, language, remote access | From `PLAN.md` (decisions 3, 7, 8, 9): web app before rules; iPhone and Android, designed for iPhone's stricter rules; Italian; Cloudflare Tunnel → Nginx Proxy Manager → host port 8100, protected by Cloudflare Access |
| — | Serving | The same binary serves the web app and the API: one container, one origin |
| N | Frontend technology | **Svelte** with **Vite**, compiled to static files that the Rust binary embeds. TypeScript, so API responses have types. Plain Svelte, not SvelteKit: no server rendering is needed and a small router of our own covers six screens (decided 2026-10-30). **Node 24 LTS** everywhere: local, Docker, CI |
| O | Look and feel | Clean, iOS-like (system font, generous spacing, rounded cards, light/dark following the phone), with a **transport character**: line badges colored by vehicle type, departure-board rows, loading indicators with a moving bus or tram. To be refined during development |
| P | Screens | Home, search, stop, near me (list, no map), favorites, info (section 5) |
| Q | Position | Asked only when tapping "vicino a me"; used for that search only, never stored |
| R | Offline | The app shell opens without network; the last loaded favorites and departures stay visible with "dati delle HH:MM, sei offline"; no offline search |
| S | Authorization | The backend accepts a request if it carries a valid **Cloudflare Access** signature (`Cf-Access-Jwt-Assertion`, checked against Cloudflare's public keys and the app's audience) **or** `API_TOKEN`. No token stored in the browser |
| T | Hostname and Access policy | Left for the deployment step: subdomain of your domain, allowed emails, session length (proposal: 30 days) |

## 3. Architecture

```
phone ──HTTPS──▶ Cloudflare (Access login) ──Tunnel──▶ Nginx Proxy Manager ──▶ :8100 container
                                                                                  │
                                              ┌───────────────────────────────────┤
                                              │ /api/*  → API (auth: Access JWT or API_TOKEN)
                                              │ /health → open
                                              │ /*      → embedded web app (index.html for unknown paths)
```

- **`web/`**: the Svelte project (Vite, TypeScript). `npm run build` writes static files to `web/dist/`.
- **Embedding**: the binary includes `web/dist/` at compile time (`rust-embed`), so the image still has a single file to run. In debug builds the files are read from disk, so a rebuild of the frontend does not need a Rust rebuild. A missing `web/dist/` does not break the Rust build: `cargo build` and `cargo test` work without Node, and the server logs a warning at startup.
- **Routing**: the app uses normal paths (`/fermata/39`); the backend answers any path that is not `/api/*`, `/health` or a file with `index.html`, and the app shows the right screen. Unknown `/api/*` paths and missing files under `/assets/` stay a real 404, so the browser never receives HTML in place of a script.
- **Caching headers**: files with a content hash in the name are cached forever; `index.html`, the manifest and the service worker are never cached, so updates arrive immediately.
- **Development**: `npm run dev` (Vite on port 5173, forwarding `/api` to `cargo run` on 8080) for instant reloads while working on the interface.

## 4. Authorization (decision S)

- **Cloudflare Access** adds `Cf-Access-Jwt-Assertion` to every request that passed its login: a JWT signed by Cloudflare.
- The backend verifies it: RS256 signature with the keys published at `https://<team>.cloudflareaccess.com/cdn-cgi/access/certs` (fetched at startup, cached, refreshed when an unknown key id appears), issuer = the team domain, audience = the Access application's AUD tag, not expired.
- A request is allowed with a valid Access JWT **or** the `API_TOKEN` bearer token (scripts, `curl`, Shortcuts). With neither configured, the API stays open with a startup warning, as today (local development).
- Static files are served without checks: they contain no data, and Access already protects them at the edge.
- If the Access session expired, an API call is redirected to the Cloudflare login and fails in the browser; the app detects it and reloads the page, which shows the login.

New settings: `CF_ACCESS_TEAM_DOMAIN` (e.g. `myteam.cloudflareaccess.com`) and `CF_ACCESS_AUD`.

## 5. Screens (decision P)

All text in Italian. Departures refresh every 30 seconds while the screen is visible and stop when the app is in the background.

| Screen | Path | Content | API |
|--------|------|---------|-----|
| Home | `/` | search bar, "vicino a me" button, favorites as cards with the next 2–3 departures each | `/api/favorites/departures` (new, section 6) |
| Search | `/cerca?q=` | results while typing (after 250 ms of pause): name, code, lines | `/api/stops?q=` |
| Stop | `/fermata/{stop}` | name, code, lines as filter chips, star to save it, live departures: line badge, destination, minutes (large), expected time, delay in color, real-time indicator, "in arrivo" at 0 minutes | `/api/stops/{stop}`, `/api/stops/{stop}/departures` |
| Near me | `/vicino` | after the location permission: stops by distance, with lines | `/api/stops/nearby` |
| Favorites | `/preferiti` | rename, choose lines, move up/down, remove | `/api/favorites` |
| Info | `/info` | version, upstream status, last stop update, how to install on the Home Screen | `/api/status` |

Phase 3 will add: "avvisami" on a departure, the live tracking page, rules, notification settings.

## 6. Backend changes

- **Static files and fallback** (section 3).
- **Authorization middleware** (section 4), replacing the token-only one; same `401 unauthorized` response.
- **`GET /api/favorites/departures?limit=3`**: the favorites with their next departures in one call, so the home screen makes one request instead of one per favorite. Uses the departures cache; a favorite whose departures fail is returned with an error flag instead of failing the whole response.
- **Settings**: `CF_ACCESS_TEAM_DOMAIN`, `CF_ACCESS_AUD`.

## 7. Look and feel (decision O)

- **Design tokens** as CSS variables: colors, spacing, radius, font sizes; one set for light, one for dark, chosen by the phone's setting.
- **Line badges**: colored by vehicle type (tram, bus, metro, rail), the line name in bold.
- **Departure rows** in the style of a departure board: line, destination, minutes aligned and large.
- **Loading**: a small bus or tram moving along a dotted line, instead of a generic spinner; skeleton rows while departures load.
- **Touch**: targets of at least 44 px, safe areas of the iPhone respected, no zoom on inputs.
- Kept deliberately simple so it can be changed later; the first screens are reviewed together before moving on.

## 8. Installable app (PWA) and offline (decision R)

- **Manifest**: name, short name, icons (192, 512, maskable), `display: standalone`, theme and background colors.
- **iPhone**: `apple-touch-icon`, `apple-mobile-web-app-capable`, status bar style, launch background color.
- **Service worker** (with `vite-plugin-pwa`, "inject manifest" mode so Phase 3 can add push handling): caches the app shell; API calls always go to the network.
- **Last data**: the app keeps the last favorites and departures it loaded (in the browser's storage) and shows them, marked with their time, when the network is missing.
- **Updates**: when a new version is deployed, the app shows "nuova versione disponibile" and reloads on tap.

## 9. Docker and CI

- **Dockerfile**: a new first stage on `node:24-slim` builds `web/dist/`; the Rust builder copies it before `cargo build`. The runtime image does not change.
- **CI**: a second job installs Node, runs `npm ci`, the Svelte type check, the frontend tests and the build. The two jobs run in parallel and independently: the Rust tests for static files use a small fixture folder, not the real build, and the Docker build is where the two meet (decided 2026-10-30).
- **`.dockerignore`**: excludes `web/node_modules` and local builds.

## 10. Deployment

1. **Hostname and Access (decision T)**: choose the subdomain; in Cloudflare Zero Trust create an Access application for it with a policy listing the allowed emails and a 30-day session; note the AUD tag and the team domain.
2. **Cloudflare Tunnel**: a public hostname for the subdomain, pointing to Nginx Proxy Manager.
3. **Nginx Proxy Manager**: a proxy host for the subdomain → server port 8100.
4. **Server**: the Phase 1 steps in [../user/deploy.md](../user/deploy.md) (Docker Hub image, folder, `./data` owned by uid 10001), with `API_TOKEN`, `CF_ACCESS_TEAM_DOMAIN` and `CF_ACCESS_AUD` in `.env`.
5. **Check**: login through Access from the phone; a direct request to port 8100 without the signature is refused; install on both phones.

Each step is done together, explaining what it does (as for the first release).

## 11. Tests

- **Rust**: Access JWT verification with a test key and a fake key server (`wiremock`): valid, expired, wrong audience, wrong issuer, unknown key, missing header, token fallback; static files (fallback to `index.html`, cache headers); `/api/favorites/departures`.
- **Frontend**: unit tests (Vitest) for formatting (minutes, delays, times) and the API client; Svelte type check.
- **By hand**: each screen on an iPhone and an Android phone, installed and in the browser; offline; dark mode.

## 12. Development order

Each step in its own branch with a PR to `main` and its chapter in `docs/development/`. When all steps are complete, `v0.2.0` is released and deployed.

1. **Frontend skeleton**: `web/` with Svelte, Vite and TypeScript; embedding and fallback in the binary; Node stage in the Dockerfile; CI job. Check: a placeholder page served by `cargo run` and by the container.
2. **Authorization**: Access JWT or `API_TOKEN`; settings.
3. **Design base**: tokens, light/dark, layout and navigation, line badges, transport loaders. Reviewed together before going on.
4. **Search and stop**: search screen, stop screen with live departures and line filters.
5. **Home and favorites**: `/api/favorites/departures`, home cards, favorites management, star on the stop screen.
6. **Near me**: location permission and nearby list.
7. **PWA and offline**: manifest, icons, service worker, last data, update prompt, iPhone details.
8. **Info screen**.
9. **Deployment** (section 10) and release `v0.2.0`.

## 13. Out of scope for this phase

- Rules, trip tracking, notifications (Phase 3)
- Map of stops or vehicles (future idea)
- Multiple users
- English interface

## 14. Risks

| Risk | Mitigation |
|------|------------|
| iPhone PWA quirks (storage limits, install flow, standalone behavior) | Tested on a real iPhone from step 7; Info screen explains how to install |
| Access session expiry breaks API calls in the installed app | 30-day session; the app detects the redirect and reloads to show the login |
| Cloudflare's keys cannot be fetched at startup | Keys cached and refreshed in the background; requests with an Access JWT are refused (not let through) until keys are available; `API_TOKEN` still works |
| Node build makes Docker builds slower | Separate cached stage; only rebuilt when `web/` changes |
| The frontend grows heavy on the phone | Svelte compiles to small files; size checked at each step |
