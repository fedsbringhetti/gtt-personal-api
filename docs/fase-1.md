# Fase 1 — Backend di base: piano dettagliato

Stato: **approvato**, decisioni confermate il 2026-10-29. Sviluppo non ancora iniziato.

## 1. Obiettivo

Un'API HTTP, in esecuzione in Docker sull'Orange Pi 5, che permetta di:
- cercare una fermata per nome, per codice o per posizione;
- vedere i prossimi passaggi di una fermata, filtrati per linea;
- salvare le fermate preferite.

### Criteri di completamento

- [ ] Cercando `porta nuova` le fermate GTT 39, 40 e 253 compaiono tra i primi risultati.
- [ ] Cercando `39` la fermata 39 è il primo risultato.
- [ ] I passaggi della fermata 39 coincidono con quelli mostrati su muoversiatorino.it.
- [ ] Gli orari restano corretti dopo mezzanotte e al cambio dell'ora legale (coperto da test).
- [ ] Se l'upstream non risponde, la ricerca fermate funziona lo stesso e i passaggi restituiscono un errore chiaro.
- [ ] Il container gira sull'Orange Pi, sopravvive a un riavvio senza perdere i dati e usa meno di 50 MB di RAM a riposo.

## 2. Decisioni

| # | Domanda | Decisione |
|---|---------|-----------|
| A | Quali feed indicizzare? | Tutti e quattro (circa 19.400 fermate, 5,7 MB, scaricati in 2 secondi); il filtro si applica in ricerca, con default solo `gtt` |
| B | Autenticazione | Nessun login; token opzionale (`API_TOKEN`) richiesto se impostato |
| C | Utenti | Un solo utente: i preferiti non sono legati a un account |
| D | Storico ritardi | Non in questa fase; lo schema non lo esclude e se ne riparla nella Fase 2 |
| E | Endpoint compatibile `/query.php?stop=` | No |
| F | Come arriva l'immagine sull'Orange Pi | Build sul Mac (`linux/arm64`, nativo su Apple Silicon) e push su GitHub Container Registry |

## 3. Cosa sappiamo dell'upstream

Verificato il 2026-10-29 su `https://plan.muoversiatorino.it/otp/routers/mato/index/graphql`.

**Feed**
| Feed | Contenuto | Fermate |
|------|-----------|---------|
| `gtt` | GTT urbano, extraurbano, ferroviario (SFM), turistico | 7.058 |
| `2` | Trenitalia | 447 |
| `3` | Arriva Italia | 1.358 |
| `4` | Autolinee regionali (GrandaBus, SAV, ecc.) | 10.559 |

**Particolarità**
- Per GTT il codice della fermata coincide con la parte dopo `gtt:`, tranne per le 23 fermate della metro (`gtt:8213`–`gtt:8227`), che hanno `code: null`. Uso l'id come codice di riserva.
- I codici si ripetono tra feed diversi (1.523 duplicati), quindi la chiave interna è sempre il `gtfsId` completo.
- I nomi sono in maiuscolo con abbreviazioni (`IVREA (V. CASALE/PONTE BREDA)`, `C.SO`, `P.ZA`): vanno normalizzati per la ricerca.
- `stops(name: ...)` restituisce sempre un elenco vuoto: la ricerca per nome va fatta in locale.
- Gli orari sono secondi da `serviceDay`, un timestamp Unix che indica l'inizio della giornata di servizio. Possono superare 86.400 per le corse dopo mezzanotte.
- Fermata inesistente: `{"data":{"stop":null}}` con HTTP 200.
- Query non valida: HTTP 500 con un array `errors`.
- Nessun header di rate limit o di cache nelle risposte.

## 4. Struttura del progetto

Un solo crate Rust.

```
gtt-personal-api/
├── Cargo.toml
├── migrations/               # migrazioni SQL (sqlx)
├── src/
│   ├── main.rs               # avvio, wiring, shutdown pulito
│   ├── config.rs             # configurazione da variabili d'ambiente
│   ├── db.rs                 # pool SQLite e migrazioni
│   ├── time.rs               # conversione serviceDay + secondi → orario Europe/Rome
│   ├── upstream/
│   │   ├── mod.rs            # client HTTP, timeout, retry, gestione errori
│   │   ├── queries.rs        # query GraphQL
│   │   └── types.rs          # tipi delle risposte
│   ├── stops/
│   │   ├── sync.rs           # download e aggiornamento dell'indice
│   │   ├── normalize.rs      # normalizzazione dei nomi
│   │   └── search.rs         # ricerca per testo, codice e posizione
│   ├── departures.rs         # passaggi e cache
│   ├── favorites.rs
│   └── api/
│       ├── mod.rs            # router
│       ├── stops.rs
│       ├── favorites.rs
│       ├── health.rs
│       └── error.rs          # formato errori JSON
├── tests/
│   └── fixtures/             # risposte upstream registrate
├── Dockerfile
└── docker-compose.yml
```

**Dipendenze principali**: `axum`, `tokio`, `reqwest` (con `rustls`, senza OpenSSL), `sqlx` (SQLite), `serde`, `chrono` + `chrono-tz`, `moka` (cache), `tracing`, `thiserror`.

## 5. Configurazione

Tutto tramite variabili d'ambiente, con valori di default sensati.

| Variabile | Default | Uso |
|-----------|---------|-----|
| `BIND_ADDR` | `0.0.0.0:8080` | indirizzo di ascolto |
| `DATA_DIR` | `/data` | cartella del database SQLite |
| `UPSTREAM_URL` | URL di muoversiatorino | endpoint GraphQL |
| `UPSTREAM_TIMEOUT_SECS` | `10` | timeout delle chiamate |
| `DEPARTURES_CACHE_SECS` | `20` | durata della cache dei passaggi |
| `STOPS_SYNC_TIME` | `04:00` | ora del sync notturno (Europe/Rome) |
| `DEFAULT_FEEDS` | `gtt` | feed usati in ricerca se non specificati |
| `API_TOKEN` | vuoto | se impostato, richiesto come `Authorization: Bearer` |
| `RUST_LOG` | `info` | livello dei log |

## 6. Client upstream

- Un'unica istanza di `reqwest::Client` condivisa, con timeout, `User-Agent` esplicito e compressione gzip.
- Due query:
  - `StopDepartures`: `stop(id)` con dettaglio e `stoptimesWithoutPatterns` (numero di passaggi e finestra temporale come parametri);
  - `AllStops`: elenco completo con coordinate, mezzo e linee.
- Variabili GraphQL sempre passate come `variables`, mai concatenate nella stringa della query.
- Errori distinti: upstream irraggiungibile, timeout, HTTP non 2xx, risposta con `errors`, fermata inesistente (`stop: null`).
- Un solo retry, con breve attesa, per gli errori di rete; nessun retry per le query non valide.

## 7. Orari

Funzione unica in `time.rs`:

```
orario = serviceDay (Unix) + secondi  →  DateTime<Europe/Rome>
```

- Gestisce i secondi oltre 86.400 senza casi speciali, perché parte dal timestamp assoluto.
- Ritardo = `realtimeDeparture - scheduledDeparture`, in secondi, anche negativo (anticipo).
- Test dedicati: corsa dopo mezzanotte, ultima domenica di marzo, ultima domenica di ottobre.

## 8. Indice delle fermate

### Schema

```sql
CREATE TABLE stops (
  id          TEXT PRIMARY KEY,   -- gtfsId, es. "gtt:39"
  feed        TEXT NOT NULL,      -- "gtt", "2", "3", "4"
  code        TEXT NOT NULL,      -- codice mostrato all'utente
  name        TEXT NOT NULL,      -- nome originale
  description TEXT,
  lat         REAL NOT NULL,
  lon         REAL NOT NULL,
  mode        TEXT,               -- BUS, TRAM, SUBWAY, RAIL, FUNICULAR
  lines       TEXT NOT NULL       -- JSON: [{"id": "...", "name": "4", "mode": "TRAM"}]
);
CREATE INDEX stops_code ON stops(code);
CREATE INDEX stops_latlon ON stops(lat, lon);

-- testo normalizzato (nome + descrizione + abbreviazioni espanse)
CREATE VIRTUAL TABLE stops_fts USING fts5(id UNINDEXED, text, tokenize = 'trigram');

CREATE TABLE sync_state (
  key   TEXT PRIMARY KEY,         -- es. "stops"
  at    TEXT NOT NULL,            -- ultimo sync riuscito
  count INTEGER NOT NULL
);
```

### Sincronizzazione
- All'avvio se l'indice è vuoto o più vecchio di 24 ore, poi ogni notte a `STOPS_SYNC_TIME`.
- Sostituzione completa dentro una sola transazione: chi cerca vede sempre o i dati vecchi o quelli nuovi, mai un indice a metà.
- Se il sync fallisce, i dati vecchi restano e si ritenta con backoff (5 minuti, 15, 1 ora).

### Normalizzazione dei nomi
- Minuscolo e rimozione degli accenti.
- Espansione delle abbreviazioni: `c.so` → `corso`, `v.` → `via`, `p.za` / `p.zza` → `piazza`, `l.go` → `largo`, `staz.` → `stazione`, `str.` → `strada`. L'elenco si completa guardando i nomi reali.
- La stessa normalizzazione si applica al testo cercato.

### Ricerca
1. Se il testo cercato è un codice, una corrispondenza esatta del codice va per prima.
2. Ricerca full-text con trigrammi (trova anche parole parziali, es. `nuov`).
3. Per testi di meno di 3 caratteri si usa `LIKE` come ripiego.
4. Ordinamento: codice esatto, poi rilevanza FTS, poi feed (`gtt` prima degli altri).

### Fermate vicine
Calcolo locale, senza chiamare l'upstream: prima un filtro rettangolare su lat/lon (usa l'indice), poi la distanza esatta (haversine) per ordinare.

## 9. Passaggi

- Chiamata all'upstream con finestra e numero di passaggi richiesti.
- Cache in memoria per fermata (`moka`, 20 secondi). Le richieste contemporanee per la stessa fermata producono una sola chiamata upstream.
- I filtri (linee, numero massimo) si applicano sulla risposta in cache, così cambiare filtro non genera nuove chiamate.
- Se l'upstream fallisce e c'è in cache una risposta di meno di 2 minuti, la restituisco con `"stale": true`; altrimenti errore.

## 10. Preferiti

```sql
CREATE TABLE favorites (
  id         INTEGER PRIMARY KEY,
  stop_id    TEXT NOT NULL,       -- gtfsId; niente foreign key, la fermata può sparire dopo un sync
  label      TEXT,                -- es. "Casa → lavoro"
  lines      TEXT,                -- JSON, filtro linee opzionale
  position   INTEGER NOT NULL,    -- ordinamento manuale
  created_at TEXT NOT NULL
);
```

Se una fermata preferita non esiste più nell'indice, la risposta la segnala con `"missing": true` invece di nasconderla.

## 11. API

Tutte le risposte sono JSON. Gli orari sono in formato ISO 8601 con fuso orario.

Nei percorsi, `{stop}` accetta il codice GTT (`39`) oppure il `gtfsId` completo (`gtt:39`, `2:S00219`).

| Metodo | Percorso | Descrizione |
|--------|----------|-------------|
| GET | `/api/stops?q=&feeds=&limit=` | ricerca per nome o codice |
| GET | `/api/stops/nearby?lat=&lon=&radius=&limit=` | fermate vicine (raggio default 300 m) |
| GET | `/api/stops/{stop}` | dettaglio fermata con le linee |
| GET | `/api/stops/{stop}/departures?lines=&minutes=&limit=` | prossimi passaggi (default 60 minuti) |
| GET | `/api/favorites` | elenco preferiti |
| POST | `/api/favorites` | aggiunge un preferito |
| PATCH | `/api/favorites/{id}` | modifica etichetta, linee o posizione |
| DELETE | `/api/favorites/{id}` | elimina un preferito |
| GET | `/health` | il processo è vivo (per Docker) |
| GET | `/api/status` | stato upstream, ultimo sync, numero di fermate |

### Esempio: passaggi

```json
{
  "stop": { "id": "gtt:39", "code": "39", "name": "PORTA NUOVA" },
  "generated_at": "2026-10-29T16:12:05+01:00",
  "stale": false,
  "departures": [
    {
      "line": "67",
      "mode": "BUS",
      "headsign": "MONCALIERI, TESTONA - PIAZZA MARCONI",
      "scheduled": "2026-10-29T15:59:00+01:00",
      "expected": "2026-10-29T16:11:29+01:00",
      "delay_seconds": 749,
      "realtime": true,
      "minutes": 0,
      "trip_id": "gtt:28459640U"
    }
  ]
}
```

### Formato degli errori

```json
{ "error": { "code": "stop_not_found", "message": "Fermata 99999 non trovata" } }
```

| Caso | HTTP | `code` |
|------|------|--------|
| Parametri non validi | 400 | `invalid_request` |
| Token mancante o errato | 401 | `unauthorized` |
| Fermata inesistente | 404 | `stop_not_found` |
| Upstream in errore | 502 | `upstream_error` |
| Upstream in timeout | 504 | `upstream_timeout` |

## 12. Docker

- **Dockerfile multi-stage**:
  - build con `rust:1.95-slim-bookworm` e `cargo-chef`, così le dipendenze restano in cache tra una build e l'altra;
  - runtime `debian:bookworm-slim` con solo `ca-certificates` e `tzdata`;
  - utente non root.
- **Health check** senza curl nell'immagine: il binario stesso ha un sottocomando `healthcheck` che chiama `/health`.
- **docker-compose.yml**: porta 8080, volume `./data:/data`, `restart: unless-stopped`, `TZ=Europe/Rome`, limite di memoria.
- Target: immagine sotto i 40 MB.

## 13. Test

- **Unitari**: conversione degli orari (mezzanotte, ora legale), normalizzazione dei nomi, ordinamento della ricerca.
- **Con fixture**: risposte reali dell'upstream salvate in `tests/fixtures/` e servite da un server finto (`wiremock`). Casi coperti: risposta normale, fermata inesistente, errore 500, timeout.
- **API**: test degli endpoint su un database SQLite temporaneo.
- **Smoke test manuale**: uno script che chiama l'upstream vero, per accorgersi quando cambia qualcosa. Non fa parte dei test automatici.
- **Qualità del codice**: `cargo fmt`, `cargo clippy` e `cargo test` in GitHub Actions a ogni push.

## 14. Ordine di sviluppo

Ogni passo termina con qualcosa di verificabile e viene sviluppato in un branch dedicato con PR verso `main`. Al completamento di tutti i passi si rilascia `v0.1.0`.

1. **Scheletro**: crate, configurazione, log, `/health`, CI. Verifica: `curl /health` risponde.
2. **Passaggi**: client upstream, conversione orari, endpoint `departures` con cache. Verifica: confronto manuale con muoversiatorino.it per la fermata 39.
3. **Indice fermate**: migrazioni, sync, normalizzazione, ricerca, fermate vicine, dettaglio. Verifica: i criteri di ricerca della sezione 1.
4. **Preferiti**: tabella ed endpoint.
5. **Robustezza**: risposte `stale`, retry, `/api/status`, token opzionale.
6. **Docker e deploy**: immagine arm64, compose, avvio sull'Orange Pi. Verifica: tutti i criteri della sezione 1 sul Pi.

## 15. Fuori da questa fase

- Regole, scheduler e notifiche (Fase 2)
- Storico dei ritardi (da decidere nella Fase 2)
- Avvisi di servizio (`alerts`): l'upstream restituisce sempre un elenco vuoto, da riverificare più avanti
- Interfaccia web (Fase 3)
- Più utenti

## 16. Rischi

| Rischio | Mitigazione |
|---------|-------------|
| L'upstream cambia schema o smette di rispondere | Errori chiari, smoke test, ricerca indipendente dall'upstream; avviso automatico nella Fase 2 |
| L'upstream inizia a limitare le richieste | Cache, una sola chiamata per fermata, sync una volta al giorno |
| Dati delle fermate incompleti (es. metro senza codice) | Codice di riserva dall'id, test sui casi noti |
