# Piano di sviluppo — gtt-personal-api

Servizio personale per i passaggi in tempo reale dei mezzi GTT a Torino, ospitato in Docker su Orange Pi 5 (ARM64).
Ispirato a [madbob/gtt-pirate-api](https://github.com/madbob/gtt-pirate-api).

Questo documento definisce le fasi. Il piano dettagliato di ogni fase viene scritto insieme, una fase alla volta, prima di iniziare a sviluppare.

## Contesto

- **Fonte dati**: API GraphQL di OpenTripPlanner su `https://plan.muoversiatorino.it/otp/routers/mato/index/graphql` (non ufficiale, può rompersi senza preavviso).
- **Cosa funziona upstream** (verificato il 2026-10-29):
  - `stop(id: "gtt:<codice>")` restituisce dettaglio e passaggi in una sola query.
  - `stopsByRadius(lat, lon, radius)` per le fermate vicine.
  - `stops { ... }` restituisce l'elenco completo (circa 7058 fermate GTT).
  - Feed disponibili: `gtt` (GTT urbano, extraurbano, SFM, turistico), `2` (Trenitalia), `3` (Arriva), `4` (autolinee regionali).
- **Cosa non funziona upstream**: `stops(name: ...)` restituisce un elenco vuoto, quindi la ricerca per nome va fatta su un indice locale.
- **Dati utili disponibili**: linea, destinazione (`headsign`), mezzo (bus/tram/metro), orario programmato e reale, `serviceDay`, id della corsa, coordinate, linee che servono una fermata.

## Stack (proposta)

- Rust: axum, tokio, reqwest, sqlx + SQLite (FTS5), serde, chrono-tz
- Immagine Docker multi-stage per `linux/arm64`, database SQLite su volume

## Decisioni trasversali

| # | Decisione | Opzioni | Scelta |
|---|-----------|---------|--------|
| 1 | Trasporti inclusi | solo GTT / tutti i feed | Tutti indicizzati, ricerca di default solo su `gtt` |
| 2 | Canale notifiche | Telegram / ntfy / Web Push (PWA) | _da decidere (Fase 2)_ |
| 3 | Accesso da fuori casa | solo LAN / Tailscale / Cloudflare Tunnel + login | _da decidere_; nel frattempo token opzionale `API_TOKEN` |
| 4 | Utenti | solo io / più persone | Solo io |
| 5 | Tipi di regole | riepilogo programmato / inseguimento corsa / "esci ora" | _da decidere (Fase 2)_ |
| 6 | Storico ritardi | sì / no | Rimandato alla Fase 2 |

## Flusso di lavoro

- **Branch**: `main` è sempre funzionante. Ogni lavoro parte da `main` in un branch breve (`feat/...`, `fix/...`, `docs/...`) e rientra con una PR, dopo che la CI (`cargo fmt`, `cargo clippy`, `cargo test`) è passata. Nessun branch `dev`.
- **Release**: tag semantici sulla `main` (`v0.1.0` a fine Fase 1, `v0.2.0` a fine Fase 2, ...). Per provare una versione sul Pi prima del rilascio si usa un tag di pre-release (`v0.2.0-rc1`).
- **Immagini Docker**: build sul Mac per `linux/arm64` e push su `ghcr.io/fedsbringhetti/gtt-personal-api`, con il tag della versione e `latest`.
- **Deploy**: sull'Orange Pi il compose punta a una versione precisa (es. `:0.1.0`), mai a `latest`. Per tornare indietro basta cambiare il tag.

---

## Fase 1 — Backend di base

**Obiettivo**: un'API affidabile per cercare fermate e leggere i passaggi, già in esecuzione sull'Orange Pi.

**Funzionalità**
- Client per l'API GraphQL upstream (una sola query per fermata)
- Copia locale delle fermate, aggiornata ogni notte, con ricerca per nome
- Ricerca delle fermate vicine a una posizione
- Dettaglio fermata con le linee che la servono
- Passaggi di una fermata: linea, destinazione, orario programmato e reale, ritardo, mezzo, filtro per linee
- Cache breve in memoria per non sovraccaricare l'upstream
- Fermate preferite
- Health check, che verifica anche la raggiungibilità dell'upstream
- Dockerfile e `docker-compose.yml` per arm64

**Piano dettagliato**: [docs/fase-1.md](docs/fase-1.md) (bozza)

---

## Fase 2 — Regole e notifiche

**Obiettivo**: ricevere automaticamente gli orari che mi servono, quando mi servono.

**Funzionalità**
- Riepilogo programmato (es. "lun–ven alle 7:40, linee 4 e 15 dalla fermata X")
- Inseguimento di una corsa (es. "il bus delle 8:05: avvisami a 5 minuti o se è in ritardo")
- Eventuale "esci ora", basato sul tempo a piedi fino alla fermata
- Invio di una regola di prova
- Avviso a me stesso quando l'upstream smette di funzionare

**Piano dettagliato**: _da scrivere_

---

## Fase 3 — Frontend

**Obiettivo**: un'interfaccia moderna e semplice, usabile dal telefono.

**Funzionalità**
- PWA installabile sul telefono, servita dallo stesso binario del backend
- Ricerca fermate per nome e per posizione
- Vista dei passaggi con aggiornamento automatico
- Gestione di preferiti e regole

**Piano dettagliato**: _da scrivere_
