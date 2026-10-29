# Phase 2 — Rules and notifications: detailed plan

Status: **draft to review**. Decisions in section 2 are proposals, not yet confirmed.

Builds on Phase 1: stop index, departures endpoint with cache, favorites.

## 1. Goal

Receive the departures I need on my phone, automatically:
- a summary of the next departures at a fixed time on chosen days;
- live tracking of a specific run I want to catch, with alerts when it is approaching, late or cancelled;
- an alert when the upstream stops working.

### Definition of done

- [ ] A Mon–Fri 7:40 summary rule sends a message within 30 seconds of 7:40, never on Saturdays, Sundays or public holidays, and never twice on the same day, even after a restart.
- [ ] Tracking "line 4 at 8:05 from stop X" sends: an alert 10 and 5 minutes before the expected departure, an alert when the delay reaches 3 minutes, and a single alert if the run is cancelled or cannot be found.
- [ ] A rule can be created, paused, resumed and tested through the API, and paused or skipped for today from Telegram.
- [ ] The Telegram bot ignores messages from any chat other than mine.
- [ ] If the upstream is down for 5 minutes I get one alert, and one message when it recovers.
- [ ] Scheduling is correct across daylight saving changes (covered by tests with a fake clock).
- [ ] Idle memory stays under 60 MB on the Orange Pi.

## 2. Decisions to confirm

| # | Question | Proposal |
|---|----------|----------|
| G | Notification channel | **Telegram bot**. It only makes outbound calls, so it works away from home without exposing the Pi; it supports buttons, commands and editing a message in place. Sending goes through a `Notifier` interface, so ntfy or Web Push can be added later |
| H | Rule types | **Summary** and **tracking**. "Leave now" is an option of tracking (walking time to the stop), not a separate type |
| I | Managing rules before the Phase 3 UI | REST API, plus a few Telegram commands for daily use (next departures, pause, skip today) |
| J | Language of notification texts | Italian (user-facing text); code, identifiers and logs stay in English |
| K | Public holidays | Rules can skip Italian national holidays plus 24 June (San Giovanni, Turin); enabled by default |
| L | Delay history | Minimal: at the end of each tracking, store the final delay of that run. Enables "this run is usually 4 minutes late" later at almost no cost |
| M | Missed schedule (Pi off or restarting at due time) | Send if at most 5 minutes late, otherwise skip and log it |

The cross-cutting decision on remote access (#3 in `PLAN.md`) does not block this phase: Telegram works without exposing the service.

## 3. What we know about the upstream

Verified on 2026-10-29.

- Every departure from `stop(...).stoptimesWithoutPatterns` carries a `trip.gtfsId` (e.g. `gtt:28440933U`).
- `trip(id).stoptimes` returns **only the static timetable** (`serviceDay: 0`, `realtime: false`).
- `trip(id).stoptimesForDate` returns **real-time** times for every stop of the run, with `serviceDay`.
- Real-time data propagates along the whole run: a run that is late at its first stop is already shown as late at later stops.
- There are **no vehicle positions** (`vehiclePositions` does not exist in the schema). A separate source exists (5T MQTT broker), recorded under "Future ideas" in `PLAN.md` and not used in this phase.

Consequence: tracking polls the stop departures (already cached in Phase 1) and identifies the run by its `trip.gtfsId`. `trip(id).stoptimesForDate` is the fallback when the run has not yet entered the stop's departure window.

## 4. Rule model

### Common fields

| Field | Example | Notes |
|-------|---------|-------|
| `name` | `"Home → work"` | |
| `kind` | `summary` / `track` | |
| `enabled` | `true` | |
| `stop_id` | `gtt:39` | gtfsId |
| `days` | `["mon","tue","wed","thu","fri"]` | days of the week |
| `skip_holidays` | `true` | see decision K |
| `paused_until` | `2026-11-03` | optional; used by "pause" and "skip today" |

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
| `tolerance_minutes` | `2` | to match the run in the timetable |
| `watch_from_minutes` | `30` | tracking starts N minutes before `scheduled_time` |
| `alert_before_minutes` | `[10, 5]` | alerts relative to the **expected** departure |
| `delay_alert_minutes` | `3` | delay threshold for an alert |
| `walk_minutes` | `6` | optional: enables "leave now" |

The run is identified each day from line, stop and scheduled time, never from a stored `trip_id`, because trip ids change between days and timetables.

## 5. Scheduler

- A single background task, ticking every 15 seconds.
- Time comes from a `Clock` trait, so tests can use a fake clock (daylight saving, midnight, holidays).
- On each tick it computes today's occurrences of every active rule in `Europe/Rome`:
  - local times that do not exist (spring forward) are moved to the next valid minute;
  - ambiguous times (fall back) use the first occurrence.
- Each occurrence has a row in `rule_runs` (rule, date, status). This makes runs idempotent: after a restart, nothing already sent is sent again.
- Calls to the upstream go through the Phase 1 departures cache, so a summary and a tracking on the same stop share the same calls.

### Summary
At `time`, fetch the departures, apply the filters, send a message. The message has a **Refresh** button that edits it with fresh data.

### Tracking: state machine

```
waiting ──(watch_from)──▶ searching ──(run found)──▶ tracking ──(departed)──▶ done
                              │                          │
                              └──(not found at T-10)──▶ not_found
                                                         └──(cancelled)──▶ cancelled
```

- **searching**: looks for the departure with the same line and a scheduled time within `tolerance_minutes`. Once found, the `trip_id` is stored for the rest of the day.
- **tracking**: polls every 30 seconds. Events, each sent at most once:
  - status message when the run is found (then edited in place with the current expected time);
  - "leave now" at `expected − walk_minutes`;
  - each threshold in `alert_before_minutes`;
  - delay reaching `delay_alert_minutes`; a new alert only if the delay changes by 3 more minutes;
  - cancellation (`realtimeState = CANCELED`).
- **done**: the expected departure has passed. If L is confirmed, the final delay is stored.
- **not_found**: 10 minutes before the scheduled time the run is still missing; one alert, then stop.
- State (current phase, `trip_id`, events already sent, last expected time) is saved in `rule_runs`, so tracking survives a restart.

Alerts are sent as new messages (they ring the phone). The status message is edited (silent).

## 6. Notifications

### `Notifier` interface

```rust
trait Notifier {
    async fn send(&self, msg: &Message) -> Result<MessageRef>;
    async fn edit(&self, msg_ref: &MessageRef, msg: &Message) -> Result<()>;
}
```

Two implementations in this phase:
- `TelegramNotifier`;
- `LogNotifier`, which only writes to the log (development and tests).

### Telegram

- Plain Bot API calls with `reqwest` (`sendMessage`, `editMessageText`, `answerCallbackQuery`, `getUpdates`). No large framework.
- Inbound updates through long polling (`getUpdates`): no webhook, no open ports.
- Only updates from `TELEGRAM_CHAT_ID` are handled; everything else is ignored and logged.
- The `getUpdates` offset is stored in the database, so no command is processed twice.
- Setup is manual: create the bot with BotFather and get the chat id (documented in the README).

### Commands and buttons

| Command | Action |
|---------|--------|
| `/next <stop> [lines]` | next departures (e.g. `/next 39 4 15`) |
| `/rules` | list rules with their state |
| `/pause <id> [days]` | pause a rule (default: until resumed) |
| `/resume <id>` | resume a rule |
| `/skip <id>` | skip today only |
| `/status` | upstream status, last stop sync, active trackings |

| Button | Where | Action |
|--------|-------|--------|
| Refresh | summary | updates the message |
| Stop tracking | tracking | ends today's tracking |
| Skip today | summary, tracking | same as `/skip` |

### Message examples (Italian, see decision J)

```
🚌 Casa → lavoro — Porta Nuova (39)
4  → Falchera        07:43 (+2)  tra 3'
15 → Sassi           07:47       tra 7'
4  → Falchera        07:55 (+1)  tra 15'
```

```
⏰ Linea 4 delle 08:05 — Porta Nuova (39)
Esci ora: passa alle 08:08 (+3), a piedi 6'.
```

## 7. Upstream health monitor

- Every upstream call reports success or failure to a shared monitor.
- If there was no traffic in the last 5 minutes, a light probe query is sent.
- Alerts:
  - **down**: failures for at least 5 minutes in a row; one message, then silence;
  - **recovered**: one message when calls succeed again;
  - **schema changed**: a GraphQL validation error (HTTP 500 with `errors`) alerts immediately, because it means the API has changed and the code needs fixing;
  - **stop sync failed**: two nightly syncs in a row failed.

## 8. Schema

```sql
CREATE TABLE rules (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  kind          TEXT NOT NULL,        -- "summary" | "track"
  enabled       INTEGER NOT NULL DEFAULT 1,
  stop_id       TEXT NOT NULL,
  days          TEXT NOT NULL,        -- JSON: ["mon", ...]
  skip_holidays INTEGER NOT NULL DEFAULT 1,
  paused_until  TEXT,                 -- date, optional
  config        TEXT NOT NULL,        -- JSON with the kind-specific fields (section 4)
  created_at    TEXT NOT NULL,
  updated_at    TEXT NOT NULL
);

CREATE TABLE rule_runs (
  rule_id    INTEGER NOT NULL REFERENCES rules(id) ON DELETE CASCADE,
  date       TEXT NOT NULL,           -- service date, Europe/Rome
  status     TEXT NOT NULL,           -- waiting, searching, tracking, done, not_found, cancelled, skipped, failed
  state      TEXT,                    -- JSON: trip_id, events sent, last expected time, status message ref
  updated_at TEXT NOT NULL,
  PRIMARY KEY (rule_id, date)
);

CREATE TABLE notifications (
  id         INTEGER PRIMARY KEY,
  rule_id    INTEGER,                 -- null for system alerts
  kind       TEXT NOT NULL,           -- summary, alert, status, system
  channel    TEXT NOT NULL,           -- "telegram", "log"
  message_ref TEXT,
  text       TEXT NOT NULL,
  sent_at    TEXT NOT NULL
);

CREATE TABLE observed_delays (        -- only if decision L is confirmed
  rule_id        INTEGER NOT NULL,
  date           TEXT NOT NULL,
  trip_id        TEXT NOT NULL,
  line           TEXT NOT NULL,
  stop_id        TEXT NOT NULL,
  scheduled      TEXT NOT NULL,
  final_expected TEXT NOT NULL,
  delay_seconds  INTEGER NOT NULL,
  PRIMARY KEY (rule_id, date)
);

CREATE TABLE kv (                     -- small internal state, e.g. Telegram update offset
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
```

`config` is JSON because the two rule kinds have different fields; it is validated with typed Rust structs on read and write.

## 9. API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/rules` | list rules with today's state |
| POST | `/api/rules` | create a rule |
| GET | `/api/rules/{id}` | rule details |
| PATCH | `/api/rules/{id}` | edit a rule |
| DELETE | `/api/rules/{id}` | delete a rule |
| POST | `/api/rules/{id}/pause` | pause, with optional `until` date |
| POST | `/api/rules/{id}/resume` | resume |
| POST | `/api/rules/{id}/test` | run the rule now and send the result |
| GET | `/api/rules/{id}/runs` | recent runs |
| GET | `/api/notifications` | sent notifications log |
| POST | `/api/notifications/test` | send a test message to the channel |

Validation errors use the Phase 1 error format (`invalid_request`, with the invalid field in the message).

## 10. Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `TELEGRAM_BOT_TOKEN` | empty | when empty, `LogNotifier` is used |
| `TELEGRAM_CHAT_ID` | empty | the only chat allowed to receive and send |
| `SCHEDULER_TICK_SECS` | `15` | scheduler tick |
| `TRACK_POLL_SECS` | `30` | polling interval during tracking |
| `MISSED_RUN_GRACE_MINUTES` | `5` | decision M |
| `UPSTREAM_DOWN_ALERT_MINUTES` | `5` | delay before the "down" alert |

## 11. Tests

- **Unit, with fake clock**: rule occurrences (weekdays, holidays, Easter Monday, pause, skip), daylight saving on both days, midnight.
- **Tracking state machine**: sequences of departures from fixtures (run on time, run getting later, run cancelled, run never appearing, restart in the middle) checked against the expected events.
- **Telegram**: fake Bot API server (`wiremock`): send, edit, commands, callbacks, chats that are not allowed.
- **Idempotency**: restart simulated between two ticks; no duplicate notification.
- **Health monitor**: failure sequences, single alert, recovery.

## 12. Development order

Each step in its own branch with a PR to `main`. When all steps are complete, `v0.2.0` is released.

1. **Rules**: migrations, typed config, CRUD API, holiday calendar, occurrence calculation with fake clock. Check: tests on occurrences.
2. **Notifier**: interface, `LogNotifier`, `TelegramNotifier` (send and edit), `/api/notifications/test`. Check: a test message arrives on the phone.
3. **Scheduler and summaries**: tick, `rule_runs`, summary rules, `/api/rules/{id}/test`. Check: summary DoD criterion.
4. **Tracking**: state machine, alerts, status message, cancellation and not found. Check: tracking DoD criterion on a real run.
5. **Telegram inbound**: long polling, chat id check, commands, buttons.
6. **Health monitor**: down, recovered, schema changed, sync failed.
7. **Delay history**: `observed_delays` (if decision L is confirmed).

## 13. Out of scope for this phase

- Web interface for rules (Phase 3)
- ntfy and Web Push channels (the interface allows them later)
- Statistics on delays (only the data is collected)
- Multiple users or multiple chats
- Rules spanning several stops or interchanges

## 14. Risks

| Risk | Mitigation |
|------|------------|
| Real-time data is missing for a run (`realtime: false`) | Alerts fall back to the scheduled time and the message says so |
| The run cannot be matched (timetable change, wrong scheduled time) | `not_found` alert with the closest departures of that line, so the rule can be fixed |
| Telegram is unreachable | Retry with backoff; alerts older than their useful time are dropped, not sent late |
| Too many upstream calls during tracking | Shared departures cache; polling only inside the watch window |
| Clock drift on the Pi | NTP is enabled on the host (check during deployment) |
