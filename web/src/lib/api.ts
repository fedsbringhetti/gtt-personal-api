/**
 * Calls to the backend API (docs/user/api.md). The types mirror the Rust structs that the
 * server serializes: `StopInfo` and `LineRef` in `src/stops/mod.rs`, `DeparturesResponse`
 * in `src/api/stops.rs`, `Departure` in `src/departures.rs`.
 */

export interface LineRef {
  id: string
  /** As people know it: `4`, `9B`, `M1`. Always a string. */
  name: string
  mode: string | null
}

export interface StopInfo {
  /** Full id, the key everywhere: `gtt:39`. */
  id: string
  feed: string
  code: string
  name: string
  description: string | null
  lat: number
  lon: number
  mode: string | null
  lines: LineRef[]
}

export interface Departure {
  line: string
  mode: string | null
  headsign: string | null
  /** ISO 8601 with the Rome offset, e.g. `2026-10-29T17:24:00+01:00`. */
  scheduled: string
  /** The time to rely on; equals `scheduled` when `realtime` is false. */
  expected: string
  /** Negative when early. */
  delay_seconds: number
  realtime: boolean
  canceled: boolean
  trip_id: string
  /** Whole minutes until `expected`, never negative, as of `generated_at`. */
  minutes: number
}

export interface DeparturesResponse {
  stop: { id: string; code: string; name: string }
  generated_at: string
  /** When the server received the data from the upstream. */
  fetched_at: string
  /** The upstream is failing and these are the last good data. */
  stale: boolean
  departures: Departure[]
}

/** How far ahead the stop screen looks, and how many rows it shows at most. */
export const DEPARTURES_MINUTES = 60
export const DEPARTURES_LIMIT = 30

/**
 * A failed call. `code` is the API's error code (`stop_not_found`, `upstream_timeout`, ...),
 * or `network` when the server could not be reached at all.
 */
export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}

async function get<T>(path: string, signal?: AbortSignal): Promise<T> {
  let response: Response
  try {
    response = await fetch(path, { signal, headers: { Accept: 'application/json' } })
  } catch (error) {
    // An aborted call is not a failure: the caller asked for it and ignores it.
    if (signal?.aborted) throw error
    throw new ApiError(0, 'network', 'Server non raggiungibile')
  }
  if (!response.ok) {
    // Errors are `{"error": {"code", "message"}}`; anything else (a proxy's HTML page) is
    // reported by its status alone.
    const body = await response.json().catch(() => null)
    const error = body?.error
    throw new ApiError(
      response.status,
      typeof error?.code === 'string' ? error.code : 'http_error',
      typeof error?.message === 'string' ? error.message : `HTTP ${response.status}`,
    )
  }
  return response.json()
}

export async function searchStops(query: string, signal?: AbortSignal): Promise<StopInfo[]> {
  const params = new URLSearchParams({ q: query, limit: '20' })
  const response = await get<{ stops: StopInfo[] }>(`/api/stops?${params}`, signal)
  return response.stops
}

export function getStop(stop: string, signal?: AbortSignal): Promise<StopInfo> {
  return get(`/api/stops/${encodeURIComponent(stop)}`, signal)
}

export function getDepartures(stop: string, signal?: AbortSignal): Promise<DeparturesResponse> {
  const params = new URLSearchParams({
    minutes: String(DEPARTURES_MINUTES),
    limit: String(DEPARTURES_LIMIT),
  })
  return get(`/api/stops/${encodeURIComponent(stop)}/departures?${params}`, signal)
}

/**
 * How a stop is written in the app's paths: the bare code for GTT (`/fermata/39`), the
 * full id for other companies, whose codes would clash with GTT's.
 */
export function stopParam(stop: Pick<StopInfo, 'id' | 'feed' | 'code'>): string {
  return stop.feed === 'gtt' ? stop.code : stop.id
}

/** What to tell the user about a failed call, in Italian. */
export function errorText(error: unknown): string {
  if (!(error instanceof ApiError)) return 'Qualcosa non ha funzionato. Riprova.'
  // A 502–504 without the API's error format comes from a proxy in front of the server
  // (Vite in development, Nginx once deployed): the server itself did not answer.
  const proxyFailure = error.code === 'http_error' && error.status >= 502 && error.status <= 504
  if (proxyFailure) return 'Server non raggiungibile. Riprova tra poco.'
  switch (error.code) {
    case 'network':
      return 'Server non raggiungibile: controlla la connessione.'
    case 'stop_not_found':
      return 'Questa fermata non esiste.'
    case 'index_not_ready':
      return "L'elenco delle fermate si sta scaricando: riprova tra un minuto."
    case 'upstream_timeout':
      return 'Il servizio degli orari è lento a rispondere. Riprova.'
    case 'upstream_error':
      return 'Il servizio degli orari non risponde. Riprova tra poco.'
    default:
      return `Qualcosa non ha funzionato (${error.status || error.code}). Riprova.`
  }
}
