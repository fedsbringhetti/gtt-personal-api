# Phase 4 — Authorization and first deployment: detailed plan

Status: **decisions confirmed**. Moved here from Phase 2 on 2026-11-02, so that the whole service (web app, GTT data, rules, notifications) is built and tested locally before it goes online. Only the hostname and the Access policy (decision T) are left for the deployment step; the plan is reviewed again before the phase starts.

Builds on Phase 1 (API, Docker image, deployment files), Phase 2 (web app, installable PWA) and Phase 3 (GTT data, rules, tracking, Web Push).

## 1. Goal

The complete service running on the home server and reachable from anywhere:
- deployed behind the existing Cloudflare Tunnel and Nginx Proxy Manager, protected by Cloudflare Access;
- checked on real phones, which need HTTPS for location, the service worker and Web Push (decision 12 in `PLAN.md`).

### Definition of done

- [ ] From outside home it is reachable only after the Cloudflare Access login; requests reaching port 8100 without the Access signature or `API_TOKEN` are refused; `/health` still works for Docker.
- [ ] It runs on the Orange Pi (Phase 1 criterion carried over), survives a restart, idle memory under 60 MB.
- [ ] On a real iPhone (web app on the Home Screen) and a real Android phone: it installs and opens full screen, "vicino a me" works, it opens offline with the last data.
- [ ] Notifications arrive on both phones (Phase 3 criterion carried over).

## 2. Decisions

| # | Topic | Decision |
|---|-------|----------|
| — | Remote access | From `PLAN.md` (decision 3): Cloudflare Tunnel → Nginx Proxy Manager → host port 8100, protected by Cloudflare Access |
| S | Authorization | The backend accepts a request if it carries a valid **Cloudflare Access** signature (`Cf-Access-Jwt-Assertion`, checked against Cloudflare's public keys and the app's audience) **or** `API_TOKEN`. No token stored in the browser |
| T | Hostname and Access policy | Left for the deployment step: subdomain of your domain, allowed emails, session length (proposal: 30 days) |

Decisions S and T were taken in the Phase 2 plan on 2026-10-30 and keep their letters.

## 3. Architecture

```
phone ──HTTPS──▶ Cloudflare (Access login) ──Tunnel──▶ Nginx Proxy Manager ──▶ :8100 container
                                                                                  │
                                              ┌───────────────────────────────────┤
                                              │ /api/*  → API (auth: Access JWT or API_TOKEN)
                                              │ /health → open
                                              │ /*      → embedded web app (index.html for unknown paths)
```

Web Push notifications are outbound calls from the server to Apple's and Google's push services: they do not go through Cloudflare Access.

## 4. Authorization (decision S)

- **Cloudflare Access** adds `Cf-Access-Jwt-Assertion` to every request that passed its login: a JWT signed by Cloudflare.
- The backend verifies it: RS256 signature with the keys published at `https://<team>.cloudflareaccess.com/cdn-cgi/access/certs` (fetched at startup, cached, refreshed when an unknown key id appears), issuer = the team domain, audience = the Access application's AUD tag, not expired.
- A request is allowed with a valid Access JWT **or** the `API_TOKEN` bearer token (scripts, `curl`, Shortcuts). With neither configured, the API stays open with a startup warning, as in local development.
- Static files are served without checks: they contain no data, and Access already protects them at the edge.
- If the Access session expired, an API call is redirected to the Cloudflare login and fails in the browser; the app detects it and reloads the page, which shows the login.

### Backend changes

- **Authorization middleware**, replacing the token-only one; same `401 unauthorized` response.
- **Settings**: `CF_ACCESS_TEAM_DOMAIN` (e.g. `myteam.cloudflareaccess.com`) and `CF_ACCESS_AUD`.

## 5. Deployment

1. **Hostname and Access (decision T)**: choose the subdomain; in Cloudflare Zero Trust create an Access application for it with a policy listing the allowed emails and a 30-day session; note the AUD tag and the team domain.
2. **Cloudflare Tunnel**: a public hostname for the subdomain, pointing to Nginx Proxy Manager.
3. **Nginx Proxy Manager**: a proxy host for the subdomain → server port 8100.
4. **Server**: the steps in [../user/deploy.md](../user/deploy.md) (Docker Hub image, folder, `./data` owned by uid 10001), with `API_TOKEN`, `CF_ACCESS_TEAM_DOMAIN`, `CF_ACCESS_AUD` and the Phase 3 secrets (VAPID key) in `.env`.
5. **Check**: login through Access from the phone; a direct request to port 8100 without the signature is refused; then the checks on real phones (section 6).

Each step is done together, explaining what it does (as for the first release).

## 6. Tests

- **Rust**: Access JWT verification with a test key and a fake key server (`wiremock`): valid, expired, wrong audience, wrong issuer, unknown key, missing header, token fallback.
- **By hand, on a real iPhone and a real Android phone**, installed and in the browser: every screen, "vicino a me", offline, dark mode, the update prompt, notifications (summary, tracking, test notification), Web Push `tag` behavior on iPhone.

## 7. Development order

Each step in its own branch with a PR to `main` and its chapter in `docs/development/`. When all steps are complete, `v0.4.0` is released and deployed.

1. **Authorization**: Access JWT or `API_TOKEN`; settings. Can be built and tested locally, with a fake key server.
2. **Deployment** (section 5), first with a pre-release (`v0.4.0-rc1`).
3. **Checks on real phones** (section 6); fixes found here go in their own `fix/...` branches before the release.

## 8. Out of scope for this phase

- Multiple users
- Any new feature: this phase only puts online what Phases 2 and 3 built

## 9. Risks

| Risk | Mitigation |
|------|------------|
| iPhone-specific issues (PWA install, storage, Web Push) found only now, since real phones are tested only in this phase (decision 12) | Development follows the iPhone's documented limits; the iOS Simulator is used during Phases 2 and 3; time for fixes is planned in step 3 |
| Access session expiry breaks API calls in the installed app | 30-day session; the app detects the redirect and reloads to show the login |
| Cloudflare's keys cannot be fetched at startup | Keys cached and refreshed in the background; requests with an Access JWT are refused (not let through) until keys are available; `API_TOKEN` still works |
| Clock drift on the server (scheduler, JWT expiry) | NTP checked during deployment |
