# Phase 3 — Rules, trip tracking and notifications: detailed plan

Status: **decisions confirmed** on 2026-10-30. The parts that depend on the web app (screens, push subscription flow) are completed once Phase 2 has settled the frontend technology; the plan is approved before the phase starts.

Until 2026-10-30 this was Phase 2, with Telegram as the notification channel. The phases were swapped so that notifications can use Web Push from the web app built in Phase 2.

Builds on Phase 1 (stop index, departures with cache, favorites, upstream health) and Phase 2 (web app, deployment behind Cloudflare Tunnel and Cloudflare Access).

## 1. Goal

Receive the departures I need on my phone, automatically, as notifications from the web app:
- a summary of the next departures at a fixed time on chosen days;
- tracking of a run I want to catch, started on the fly from the departures list or by a recurring rule, with reminders and alerts when it is late or cancelled, plus a live page;
- an alert when the upstream stops working;
- a history of the delays of tracked runs.

### Definition of done

- [ ] A Mon–Fri 7:40 summary rule sends a notification within 30 seconds of 7:40, never on Saturdays, Sundays or public holidays, and never twice on the same day, even after a restart.
- [ ] Tapping a departure in the web app and choosing "avvisami" tracks that run today: reminders 10 and 5 minutes before the expected departure, an alert when the delay reaches 3 minutes, a single alert if it is cancelled or cannot be found.
- [ ] A recurring tracking rule ("line 4 at 8:05 from stop X, Mon–Fri") does the same every matching day.
- [ ] The live page of a tracked run shows the expected time and the countdown, updated while it is open.
- [ ] Rules can be created, edited, paused, skipped for today, resumed, tested and deleted from the web app.
- [ ] Notifications arrive on both an iPhone (web app on the Home Screen) and an Android phone.
- [ ] If the upstream is down for 5 minutes there is one alert, and one notification when it recovers.
- [ ] Scheduling is correct across daylight saving changes (covered by tests with a fake clock).
- [ ] Idle memory stays under 60 MB in the container.

## 2. Decisions

| # | Question | Decision |
|---|----------|----------|
| G | Notification channel | **Web Push** from the web app (no Telegram, no third-party app). Sending goes through a `Notifier` interface; a `LogNotifier` is used in development and tests |
| H | Rule types | **Summary** and **tracking**. Tracking can start **on the fly** (from the departures list, for today) or from a **recurring rule**. Events: reminders N minutes before the expected departure (chosen by the user, default 10 and 5), delay over a threshold, cancellation, run not found; plus a live page. **No "leave now"**: walking time is not computed (future idea in `PLAN.md`) |
| I | Managing rules | From the web app, through the API |
| J | Language of user-facing text | Italian (notifications and web app); code, identifiers, logs and docs in English |
| K | Public holidays | Rules can skip Italian national holidays plus 24 June (San Giovanni, Turin), enabled by default per rule. Personal dates are a future idea in `PLAN.md` |
| L | Delay history | Yes, for tracked runs only, stored **per run** (service date, trip, stop), not per rule or per user, so two trackings of the same run share one record and future users would share the data. To be evaluated after some weeks of use |
| M | Missed schedule (server off or restarting at due time) | Summaries: sent if at most 5 minutes late, otherwise skipped and logged. Tracking: resumes from its saved state; an alert that is no longer useful (the bus has left) is dropped, never sent late |

## 3. What we know about the upstream

Verified on 2026-10-29.

- Every departure from `stop(...).stoptimesWithoutPatterns` carries a `trip.gtfsId` (e.g. `gtt:28440933U`).
- `trip(id).stoptimes` returns **only the static timetable** (`serviceDay: 0`, `realtime: false`).
- `trip(id).stoptimesForDate` returns **real-time** times for every stop of the run, with `serviceDay`.
- Real-time data propagates along the whole run: a run that is late at its first stop is already shown as late at later stops. Before the vehicle starts its run there is usually only the scheduled time.
- There are **no vehicle positions** (`vehiclePositions` does not exist in the schema). A separate source exists (5T MQTT broker), recorded under "Future ideas" in `PLAN.md` and not used in this phase.

Consequence: tracking polls the stop departures (already cached since Phase 1) and identifies the run by its `trip.gtfsId`. `trip(id).stoptimesForDate` is the fallback when the run has not yet entered the stop's departure window.

## 4. Rule model

### Common fields

| Field | Example | Notes |
|-------|---------|-------|
| `name` | `"Casa → lavoro"` | |
| `kind` | `summary` / `track` | |
| `enabled` | `true` | |
| `stop_id` | `gtt:39` | gtfsId |
| `days` | `["mon","tue","wed","thu","fri"]` | days of the week; empty for a one-off tracking |
| `date` | `2026-11-03` | only for a one-off tracking started on the fly |
| `skip_holidays` | `true` | decision K |
| `paused_until` | `2026-11-03` | optional; used by "pause" and "skip today" |

A tracking started on the fly is stored as a one-off `track` rule with `date` set and no `days`; it is removed automatically when it ends. Everything else (scheduler, state, history) is shared with recurring rules.

### Summary rule

| Field | Example | Notes |
|-------|---------|-------|
| `time` | `07:40` | Europe/Rome |
| `lines` | `["4", "15"]` | optional filter |
| `window_minutes` | `30` | departures shown in the next N minutes |
| `max_departures` | `6` | |

### Tracking rule

| Field | Example | Notes |
|-------|---------|-------|
| `line` | `"4"` | a single line |
| `scheduled_time` | `08:05` | scheduled departure at the stop |
| `trip_id` | `gtt:28440933U` | one-off tracking only: the run tapped in the departures list |
| `tolerance_minutes` | `2` | recurring rules: to match the run in the timetable |
| `watch_from_minutes` | `30` | tracking starts N minutes before `scheduled_time` |
| `alert_before_minutes` | `[10, 5]` | reminders relative to the **expected** departure |
| `delay_alert_minutes` | `3` | delay threshold for an alert |

A recurring rule identifies the run each day from line, stop and scheduled time, never from a stored `trip_id`, because trip ids change between days and timetables.

## 5. Scheduler

- A single background task, ticking every 15 seconds.
- Time comes from the `Clock` trait, so tests use a fake clock (daylight saving, midnight, holidays).
- On each tick it computes today's occurrences of every active rule in `Europe/Rome`: local times that do not exist (spring forward) move to the next valid minute; ambiguous ones (fall back) use the first occurrence.
- Each occurrence has a row in `rule_runs` (rule, date, status), so after a restart nothing already sent is sent again.
- Upstream calls go through the departures cache, so a summary and a tracking on the same stop share the same calls.

### Summary
At `time`, fetch the departures, apply the filters, send a notification listing them. Tapping it opens the departures page of that stop in the web app, which is live.

### Tracking: state machine

```
waiting ──(watch_from)──▶ searching ──(run found)──▶ tracking ──(departed)──▶ done
                              │                          │
                              └──(not found at T-10)──▶ not_found
                                                         └──(cancelled)──▶ cancelled
```

- **searching**: recurring rules look for the departure with the same line and a scheduled time within `tolerance_minutes`; one-off trackings already know the `trip_id`. Once found, the `trip_id` is stored for the rest of the day.
- **tracking**: polls every 30 seconds. Events, each notified at most once:
  - each threshold in `alert_before_minutes`, relative to the expected departure;
  - delay reaching `delay_alert_minutes`; a new alert only if the delay grows by 3 more minutes;
  - cancellation (`realtimeState = CANCELED`), with the next departure of the same line.
- **done**: the expected departure has passed; the final delay is written to the delay history (decision L).
- **not_found**: 10 minutes before the scheduled time the run is still missing; one alert with the closest departures of that line, then stop.
- State (phase, `trip_id`, events already notified, last expected time) is saved in `rule_runs`, so tracking survives a restart.
- Notifications of the same tracking share a Web Push `tag`, so a newer one replaces the older one instead of piling up (to verify on iPhone).

### Live page
`GET /api/trackings/{id}` returns the current state of a tracking (phase, scheduled and expected time, delay, realtime flag). The web app polls it while the page is open and shows a countdown; no notification is needed for routine updates.

## 6. Notifications (Web Push)

### `Notifier` interface

```rust illustrative
trait Notifier {
    async fn send(&self, notification: &Notification) -> Result<(), NotifyError>;
}
```

Implementations: `WebPushNotifier` and `LogNotifier` (development and tests).

### How Web Push works here

1. The server has a **VAPID key pair**, generated once (`gtt-personal-api vapid-keys` subcommand); the private key lives in `.env`.
2. The web app, after the user taps "attiva notifiche", asks the browser for a push subscription with the server's public key and sends it to `POST /api/push/subscriptions`. On iPhone this works only when the web app was added to the Home Screen (iOS 16.4+), and the permission prompt must follow a tap.
3. To notify, the server encrypts the payload and posts it to the subscription's endpoint (Apple's or Google's push service). This is an outbound call: it does not go through Cloudflare Access.
4. The service worker in the web app shows the notification; tapping it opens the right page (departures of a stop, live page of a tracking).
5. Subscriptions that the push service reports as gone (HTTP 404 or 410) are deleted.

Library: a Rust Web Push crate with VAPID and `aes128gcm` support, to choose when the phase starts (`web-push` is the obvious candidate; check that it works with our `reqwest`/`rustls` setup).

### Message examples (Italian, decision J)

```text
🚌 Casa → lavoro — Porta Nuova (39)
4  → Falchera   07:43 (+2)  tra 3'
15 → Sassi      07:47       tra 7'
```

```text
⏰ Il 4 delle 08:05 passa tra 5 minuti (08:08, +3)
```

```text
⚠️ Il 4 delle 08:05 è cancellato. Il prossimo: 08:17
```

## 7. Upstream health alerts

Built on `UpstreamHealth` from Phase 1:

- if there was no traffic in the last 5 minutes, a light probe query is sent;
- **down**: failures for at least 5 minutes in a row; one notification, then silence;
- **recovered**: one notification when calls succeed again;
- **schema changed**: a GraphQL validation error alerts immediately, because the code needs fixing;
- **stop sync failed**: two nightly syncs in a row failed.

## 8. Schema

```sql illustrative
CREATE TABLE rules (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  kind          TEXT NOT NULL,        -- "summary" | "track"
  enabled       INTEGER NOT NULL DEFAULT 1,
  stop_id       TEXT NOT NULL,
  days          TEXT NOT NULL,        -- JSON: ["mon", ...]; empty for one-off trackings
  date          TEXT,                 -- one-off trackings only
  skip_holidays INTEGER NOT NULL DEFAULT 1,
  paused_until  TEXT,
  config        TEXT NOT NULL,        -- JSON with the kind-specific fields (section 4)
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);

CREATE TABLE rule_runs (
  rule_id    INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
  date       TEXT NOT NULL,           -- service date, Europe/Rome
  status     TEXT NOT NULL,           -- waiting, searching, tracking, done, not_found, cancelled, skipped, failed
  state      TEXT,                    -- JSON: trip_id, events notified, last expected time
  updated_at TEXT NOT NULL,
  PRIMARY KEY (rule_id, date)
);

CREATE TABLE push_subscriptions (
  id         INTEGER PRIMARY KEY,
  endpoint   TEXT NOT NULL UNIQUE,
  p256dh     TEXT NOT NULL,
  auth       TEXT NOT NULL,
  label      TEXT,                    -- e.g. "iPhone", from the user agent
  created_at TEXT NOT NULL
);

CREATE TABLE notifications (
  id       INTEGER PRIMARY KEY,
  rule_id  INTEGER,                   -- null for system alerts
  kind     TEXT NOT NULL,             -- summary, reminder, delay, cancelled, not_found, system
  channel  TEXT NOT NULL,             -- "webpush", "log"
  text     TEXT NOT NULL,
  sent_at  TEXT NOT NULL
);

-- One row per tracked run: shared by every rule (and, later, every user) tracking it.
CREATE TABLE observed_delays (
  service_date   TEXT NOT NULL,
  trip_id        TEXT NOT NULL,
  stop_id        TEXT NOT NULL,
  line           TEXT NOT NULL,
  scheduled      TEXT NOT NULL,
  final_expected TEXT NOT NULL,
  delay_seconds  INTEGER NOT NULL,
  realtime       INTEGER NOT NULL,     -- whether the final time came from real-time data
  PRIMARY KEY (service_date, trip_id, stop_id)
);
```

`config` is JSON because the two rule kinds have different fields; it is validated with typed Rust structs on read and write.

## 9. API

| Method | Path | Description |
|--------|------|-------------|
| GET, POST | `/api/rules` | list rules with today's state; create a rule |
| GET, PATCH, DELETE | `/api/rules/{id}` | details, edit, delete |
| POST | `/api/rules/{id}/pause` | pause, with optional `until` date ("skip today" = until tomorrow) |
| POST | `/api/rules/{id}/resume` | resume |
| POST | `/api/rules/{id}/test` | run the rule now and send the result |
| GET | `/api/rules/{id}/runs` | recent runs |
| POST | `/api/trackings` | track a run on the fly: `{"stop": "39", "trip_id": "gtt:..."}` |
| GET, DELETE | `/api/trackings/{id}` | live state of a tracking; stop it |
| GET | `/api/push/key` | VAPID public key for the web app |
| POST, DELETE | `/api/push/subscriptions` | register or remove this device |
| POST | `/api/notifications/test` | send a test notification to every device |
| GET | `/api/notifications` | sent notifications log |
| GET | `/api/delays?stop=&line=&time=` | delay statistics of a run (average, worst, number of days) |

Validation errors use the Phase 1 error format.

## 10. Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `VAPID_PRIVATE_KEY` | empty | Web Push key (secret, in `.env`); when empty, `LogNotifier` is used |
| `VAPID_SUBJECT` | empty | contact for push services, e.g. `mailto:...` |
| `SCHEDULER_TICK_SECS` | `15` | scheduler tick |
| `TRACK_POLL_SECS` | `30` | polling interval during tracking |
| `MISSED_RUN_GRACE_MINUTES` | `5` | decision M |
| `UPSTREAM_DOWN_ALERT_MINUTES` | `5` | delay before the "down" alert |

## 11. Tests

- **Unit, with fake clock**: rule occurrences (weekdays, holidays, Easter Monday, pause, skip, one-off dates), daylight saving on both days, midnight.
- **Tracking state machine**: sequences of departures from fixtures (on time, getting later, cancelled, never appearing, restart in the middle) checked against the expected events.
- **Web Push**: a fake push service (`wiremock`): payload sent to every subscription, 404/410 removes it.
- **Idempotency**: restart between two ticks; no duplicate notification.
- **Health alerts**: failure sequences, single alert, recovery.
- **By hand**: notifications on a real iPhone (Home Screen) and a real Android phone.

## 12. Development order

Each step in its own branch with a PR to `main` and its chapter in `docs/development/`. When all steps are complete, `v0.3.0` is released.

1. **Rules**: migrations, typed config, CRUD API, holiday calendar, occurrence calculation with fake clock.
2. **Web Push**: VAPID keys, subscriptions, `WebPushNotifier`, `LogNotifier`, service worker and "attiva notifiche" in the web app, test notification. Check: a test notification arrives on both phones.
3. **Scheduler and summaries**: tick, `rule_runs`, summary rules, `/api/rules/{id}/test`.
4. **Tracking**: state machine, one-off trackings from the departures list, recurring tracking rules, live page.
5. **Rules in the web app**: list, create, edit, pause, skip today, delete.
6. **Health alerts**: down, recovered, schema changed, sync failed.
7. **Delay history**: `observed_delays` and `/api/delays`, shown next to tracked runs.

## 13. Out of scope for this phase

- Telegram, ntfy and other channels (the `Notifier` interface allows them later)
- Walking time and "leave now" (future idea)
- Personal dates to skip (future idea)
- Vehicle positions (future idea)
- Multiple users
- Rules spanning several stops or interchanges

## 14. Risks

| Risk | Mitigation |
|------|------------|
| Real-time data is missing for a run (`realtime: false`) | Alerts use the scheduled time and say so |
| The run cannot be matched (timetable change, wrong scheduled time) | `not_found` alert with the closest departures of that line, so the rule can be fixed |
| iPhone Web Push limits (Home Screen only, permission after a tap, `tag` behavior) | Designed in from Phase 2 (installable PWA); verified on a real iPhone in step 2 |
| Cloudflare Access session expires and the web app shows the Access login | Long session duration in the Access policy; the web app detects it and asks to log in again |
| A push subscription expires | Removed on 404/410; the web app re-subscribes when opened |
| Too many upstream calls during tracking | Shared departures cache; polling only inside the watch window |
| Clock drift on the server | NTP checked during deployment |
