# CLAUDE.md

Servizio personale per i passaggi in tempo reale dei mezzi GTT a Torino: backend in Rust, eseguito in Docker su un Orange Pi 5 (ARM64). Un solo utente.

## Documenti di riferimento

- [PIANO.md](PIANO.md): fasi del progetto, decisioni trasversali, flusso di lavoro.
- [docs/fase-1.md](docs/fase-1.md): piano dettagliato della Fase 1 (backend di base).

Le decisioni prese sono registrate in questi file: seguile, e se una scelta non è coperta chiedi invece di decidere da solo.

## Come lavoriamo

- Prima si pianifica, poi si sviluppa. Non scrivere codice, scheletri di progetto o file di configurazione per una fase finché non c'è il via esplicito.
- Ogni fase ha un piano dettagliato in `docs/fase-N.md`, scritto e approvato prima di iniziarla. Se durante lo sviluppo il piano cambia, aggiorna il file.
- Documentazione e testi del progetto in italiano.

## Git

- `main` è sempre funzionante. Nessun branch `dev`.
- Ogni lavoro in un branch breve da `main` (`feat/...`, `fix/...`, `docs/...`), che rientra con una PR dopo che la CI è passata.
- Release con tag semantici (`v0.1.0`, ...); pre-release con `-rcN`.
- Commit, push, PR e tag solo su richiesta.

## Stack

- Rust: `axum`, `tokio`, `reqwest` (rustls), `sqlx` + SQLite (FTS5), `serde`, `chrono` + `chrono-tz`, `moka`, `tracing`, `thiserror`.
- Docker multi-stage per `linux/arm64`; immagini su `ghcr.io/fedsbringhetti/gtt-personal-api`.

## Fonte dati

API GraphQL di OpenTripPlanner su `https://plan.muoversiatorino.it/otp/routers/mato/index/graphql`. Non è ufficiale e può cambiare senza preavviso. Punti da ricordare:

- Una fermata si interroga con `stop(id: "gtt:<codice>")`, che restituisce anche i passaggi: non serve una seconda query.
- `stops(name: ...)` non funziona (restituisce sempre un elenco vuoto): la ricerca per nome si fa sull'indice locale.
- Orario di un passaggio = `serviceDay` (Unix) + secondi, poi conversione in `Europe/Rome`. I secondi possono superare 86.400. Non usare "oggi + secondi".
- I codici fermata si ripetono tra feed diversi: la chiave è sempre il `gtfsId` completo (`gtt:39`).
- Le fermate della metro GTT hanno `code: null`: il codice si ricava dall'id.
- Le linee non sono numeri (`9B`, `M1S`, `68+`): trattale sempre come stringhe.
- Passare i parametri come `variables` GraphQL, mai concatenati nella query.

## Comandi

Il codice non esiste ancora. Quando ci sarà, prima di ogni PR:

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
