# gtt-personal-api

Personal service for real-time GTT public transport departures in Turin: stop search by name and position, real-time departures with line filters, favorite stops. A Rust backend with SQLite, packaged as a Docker image for an Orange Pi 5.
Inspired by [madbob/gtt-pirate-api](https://github.com/madbob/gtt-pirate-api).

**Status**: Phase 1 (core backend) done. Phase 2 (rules and Telegram notifications) is being planned; Phase 3 adds a web interface and the first deployment. See [PLAN.md](PLAN.md).

## Quick start

From source:

```bash
cargo run
```

```bash
curl -s 'http://localhost:8080/api/stops?q=porta%20nuova&limit=3'
```

```bash
curl -s 'http://localhost:8080/api/stops/39/departures?limit=5'
```

With Docker (local testing):

```bash
scripts/build-image.sh
```

```bash
cp .env.example .env
```

```bash
docker compose up -d
```

The service is then on `http://localhost:8100`; requests need `Authorization: Bearer <API_TOKEN from .env>`.

## Documentation

Start at [docs/README.md](docs/README.md).

| Topic | Document |
|-------|----------|
| Every endpoint, with examples | [docs/user/api.md](docs/user/api.md) |
| Environment variables | [docs/user/configuration.md](docs/user/configuration.md) |
| Docker image and deployment | [docs/user/deploy.md](docs/user/deploy.md) |
| How it works, code map, limits | [docs/architecture.md](docs/architecture.md) |
| How it was built, step by step | [docs/development/](docs/development/README.md) |

## Checks

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
