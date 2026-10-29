# gtt-personal-api

Personal service for real-time GTT public transport departures in Turin, meant to run in Docker on an Orange Pi 5.
Inspired by [madbob/gtt-pirate-api](https://github.com/madbob/gtt-pirate-api).

See [PLAN.md](PLAN.md) for phases and decisions.

## Running locally

```bash
cargo run
```

```bash
curl http://localhost:8080/health
```

## Configuration

| Variable | Default | Purpose |
|----------|---------|---------|
| `BIND_ADDR` | `0.0.0.0:8080` | listen address |
| `RUST_LOG` | `info` | log level |

## Checks

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
