# CLAUDE.md

Personal service for real-time GTT public transport departures in Turin: Rust backend, running in Docker on an Orange Pi 5 (ARM64). Single user.

## Reference documents

- [PLAN.md](PLAN.md): project phases, cross-cutting decisions, workflow.
- [docs/phase-1.md](docs/phase-1.md): detailed plan for Phase 1 (core backend).

Decisions already made are recorded in these files: follow them, and when a choice is not covered, ask instead of deciding alone.

## Language

- Everything in the repository is in English: file and directory names, document contents, code, identifiers, comments, branch names, commit messages, PR titles and descriptions, release notes.
- Conversation with the user is in Italian.

## How we work

- Plan first, then build. Do not write code, project skeletons or configuration files for a phase until the user explicitly says to start.
- Each phase has a detailed plan in `docs/phase-N.md`, written and approved before the phase starts. If the plan changes during development, update the file.

## Git

- `main` always works. No `dev` branch.
- Every change in a short-lived branch from `main` (`feat/...`, `fix/...`, `docs/...`), merged back through a PR once CI passes.
- Releases as semantic version tags (`v0.1.0`, ...); pre-releases with `-rcN`.
- Commit, push, open PRs and tag only when asked.

## Stack

- Rust: `axum`, `tokio`, `reqwest` (rustls), `sqlx` + SQLite (FTS5), `serde`, `chrono` + `chrono-tz`, `moka`, `tracing`, `thiserror`.
- Multi-stage Docker for `linux/arm64`; images on `ghcr.io/fedsbringhetti/gtt-personal-api`.

## Data source

OpenTripPlanner GraphQL API at `https://plan.muoversiatorino.it/otp/routers/mato/index/graphql`. It is unofficial and may change without notice. Things to remember:

- Query a stop with `stop(id: "gtt:<code>")`, which also returns departures: no second query is needed.
- `stops(name: ...)` does not work (always returns an empty list): name search uses the local index.
- Departure time = `serviceDay` (Unix) + seconds, then converted to `Europe/Rome`. Seconds can exceed 86,400. Never use "today + seconds".
- Stop codes repeat across feeds: the key is always the full `gtfsId` (`gtt:39`).
- GTT metro stops have `code: null`: derive the code from the id.
- Lines are not numbers (`9B`, `M1S`, `68+`): always treat them as strings.
- Pass parameters as GraphQL `variables`, never concatenated into the query.

## Commands

There is no code yet. Once there is, before every PR:

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
