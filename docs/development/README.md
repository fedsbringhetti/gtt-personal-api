# Development

How the service was built, one chapter per development step, in order. Each chapter explains the goal, the decisions and why they were made, the code with short excerpts, the tests, how to check it by hand, and the pitfalls met along the way.

Read the chapters in order the first time: each builds on the previous ones. Code excerpts show the code as it is today; when a later step changed something, the chapter says so.

## Chapters

| # | Chapter | Phase | PR |
|---|---------|-------|----|
| 1 | [Project skeleton](01-skeleton.md): crate, configuration, logging, `/health`, CI | 1 | [#3](https://github.com/fedsbringhetti/gtt-personal-api/pull/3) |
| 2 | [Departures](02-departures.md): upstream client, times, cache, first real endpoint | 1 | [#4](https://github.com/fedsbringhetti/gtt-personal-api/pull/4) |
| 3 | [Stop index](03-stop-index.md): SQLite, nightly sync, name normalization, search, nearby | 1 | [#5](https://github.com/fedsbringhetti/gtt-personal-api/pull/5) |
| 4 | [Favorites](04-favorites.md): ordered favorites, JSON bodies, partial updates | 1 | [#6](https://github.com/fedsbringhetti/gtt-personal-api/pull/6) |
| 5 | [Robustness](05-robustness.md): retry, stale data, status, optional token | 1 | [#7](https://github.com/fedsbringhetti/gtt-personal-api/pull/7) |
| 6 | [Docker](06-docker.md): image, health check, compose, deployment files | 1 | [#8](https://github.com/fedsbringhetti/gtt-personal-api/pull/8) |

The planning that came before the code is in [../plan/](../plan/phase-1.md); the overall picture is in [../architecture.md](../architecture.md).

## Workflow

### Branches and PRs

- `main` always works. Every step is a short-lived branch from `main` (`feat/...`, `fix/...`, `docs/...`) merged through a PR once CI passes.
- Everything in the repository is in English: code, comments, docs, branch names, commits, PR titles and descriptions.
- Releases are semantic version tags on `main` (`v0.1.0`, ...).

### Checks

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

The same checks run in GitHub Actions (`.github/workflows/ci.yml`) on every PR and on every push to `main`.

### Tests

- **Unit tests** sit at the bottom of each source file in a `#[cfg(test)] mod tests` block. They test pure functions: times, normalization, ranking helpers, config parsing, sync scheduling, token parsing.
- **Integration tests** live in `tests/`, one file per area (`departures.rs`, `stops.rs`, `favorites.rs`, `status.rs`, `auth.rs`, `health.rs`). They build the real router with an in-memory database, a fixed clock and a fake upstream server (`wiremock`), then send HTTP requests to it.
- **Shared helpers** are in `tests/common/mod.rs`: `test_app` / `test_app_with` (app with options), `synced_app` (app with the stop index already loaded), `get`, `send`, `request`.
- **Fixtures** in `tests/fixtures/` are real upstream responses saved to files: `stop_39.json` (departures at Porta Nuova) and `all_stops.json` (44 real stops).

### Common changes

| Change | Where |
|--------|-------|
| New endpoint | handler in `src/api/`, route in `src/api/mod.rs` (it is behind the token automatically), error codes in `src/api/error.rs`, integration test in `tests/` |
| Database change | new `migrations/000N_description.sql`, applied at the next startup; never edit a migration that already ran |
| New upstream field | query in `src/upstream/queries.rs`, shape in `src/upstream/types.rs`, refresh fixtures if tests need it |
| New setting | `Config` in `src/config.rs` with a default and a test, then [../user/configuration.md](../user/configuration.md) and `.env.example` |

## Writing a chapter

Every development step adds a chapter here, in the same PR as the code. Name it `NN-short-name.md` and add it to the table above. Use this outline:

1. **Goal**: what the step delivers, in two or three sentences.
2. **Decisions**: each choice made and why, including alternatives rejected.
3. **Files**: table of files added or changed, with their role.
4. **How the code works**: walkthrough in the order a request or process runs, with short excerpts of the real code and the Rust concepts they use.
5. **Tests**: what is tested and how.
6. **Check it by hand**: commands to see it working.
7. **Pitfalls**: problems met and how they were solved.
8. **Later changes**: links to later chapters that changed this code.

When behavior visible to users changes, also update [../user/api.md](../user/api.md), [../user/configuration.md](../user/configuration.md) or [../architecture.md](../architecture.md).
