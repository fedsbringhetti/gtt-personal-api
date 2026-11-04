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

/** A stop of `GET /api/stops/nearby` (`NearbyStop` in `src/stops/search.rs`). */
export interface NearbyStop extends StopInfo {
  /** Straight-line distance in meters. */
  distance_m: number
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
  /** Where `expected` comes from: GTT's own prediction, OpenTripPlanner's, or the timetable. */
  source: 'gtt' | 'otp' | 'scheduled'
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

/** A favorite as `GET /api/favorites` returns it (`FavoriteView` in `src/api/favorites.rs`). */
export interface Favorite {
  id: number
  stop_id: string
  /** The name chosen by the user; `null` shows the stop name. */
  label: string | null
  /** Lines to show; empty for all. */
  lines: string[]
  position: number
  created_at: string
  /** `null` when the stop is not in the index (see `missing`). */
  stop: StopInfo | null
  /** A sync removed the stop. */
  missing: boolean
}

export interface FavoriteDepartures extends Favorite {
  departures: Departure[]
  stale: boolean
  /** Why this favorite has no departures; the others are not affected. */
  error: { code: string; message: string } | null
}

/** Response of `GET /api/status` (`StatusResponse` in `src/api/status.rs`). */
export interface Status {
  version: string
  time: string
  upstream: {
    /** `null` before the first call to the upstream. */
    ok: boolean | null
    last_success: string | null
    last_failure: string | null
    last_error: string | null
    consecutive_failures: number
  }
  /** `null` until the first stop sync. */
  stop_index: { synced_at: string; count: number } | null
  /** GTT's real-time data; `null` when the server has it disabled. */
  gtt: {
    trip_updates: Status['upstream']
    /** The last trip updates read; `null` until the first success. */
    feed: { runs: number; newest_update: string | null } | null
    /** Today's comparison with OpenTripPlanner, one entry per run and stop. */
    comparison: {
      date: string
      used: number
      mismatches: number
      compared: number
      median_difference_seconds: number | null
    }
  } | null
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

interface RequestOptions {
  method?: 'GET' | 'POST' | 'PATCH' | 'DELETE'
  /** Sent as JSON. */
  body?: unknown
  signal?: AbortSignal
}

async function request<T>(
  path: string,
  { method = 'GET', body, signal }: RequestOptions = {},
): Promise<T> {
  const headers: Record<string, string> = { Accept: 'application/json' }
  if (body !== undefined) headers['Content-Type'] = 'application/json'
  let response: Response
  try {
    response = await fetch(path, {
      method,
      signal,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    })
  } catch (error) {
    // An aborted call is not a failure: the caller asked for it and ignores it.
    if (signal?.aborted) throw error
    throw new ApiError(0, 'network', 'Server non raggiungibile')
  }
  if (!response.ok) {
    // Errors are `{"error": {"code", "message"}}`; anything else (a proxy's HTML page) is
    // reported by its status alone.
    const answer = await response.json().catch(() => null)
    const error = answer?.error
    throw new ApiError(
      response.status,
      typeof error?.code === 'string' ? error.code : 'http_error',
      typeof error?.message === 'string' ? error.message : `HTTP ${response.status}`,
    )
  }
  // 204 No Content (a delete) has no body to read.
  return response.status === 204 ? (undefined as T) : response.json()
}

function get<T>(path: string, signal?: AbortSignal): Promise<T> {
  return request(path, { signal })
}

export async function searchStops(query: string, signal?: AbortSignal): Promise<StopInfo[]> {
  const params = new URLSearchParams({ q: query, limit: '20' })
  const response = await get<{ stops: StopInfo[] }>(`/api/stops?${params}`, signal)
  return response.stops
}

/** Stops within `radius` meters of a point, nearest first. */
export async function nearbyStops(
  position: { lat: number; lon: number },
  radius: number,
  signal?: AbortSignal,
): Promise<NearbyStop[]> {
  const params = new URLSearchParams({
    lat: String(position.lat),
    lon: String(position.lon),
    radius: String(radius),
  })
  return (await get<{ stops: NearbyStop[] }>(`/api/stops/nearby?${params}`, signal)).stops
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

export function getStatus(signal?: AbortSignal): Promise<Status> {
  return get('/api/status', signal)
}

/** Departures per favorite on the home screen. */
export const HOME_DEPARTURES = 3

export async function getFavorites(signal?: AbortSignal): Promise<Favorite[]> {
  return (await get<{ favorites: Favorite[] }>('/api/favorites', signal)).favorites
}

export async function getFavoritesDepartures(signal?: AbortSignal) {
  const params = new URLSearchParams({ limit: String(HOME_DEPARTURES) })
  return get<{ generated_at: string; favorites: FavoriteDepartures[] }>(
    `/api/favorites/departures?${params}`,
    signal,
  )
}

export function addFavorite(stop: string, lines: string[]): Promise<Favorite> {
  return request('/api/favorites', { method: 'POST', body: { stop, lines } })
}

/** Changes only the fields given; `label: null` goes back to the stop name. */
export function updateFavorite(
  id: number,
  changes: { label?: string | null; lines?: string[]; position?: number },
): Promise<Favorite> {
  return request(`/api/favorites/${id}`, { method: 'PATCH', body: changes })
}

export function removeFavorite(id: number): Promise<void> {
  return request(`/api/favorites/${id}`, { method: 'DELETE' })
}

/**
 * How a stop is written in the app's paths: the bare code for GTT (`/fermata/39`), the
 * full id for other companies, whose codes would clash with GTT's.
 */
export function stopParam(stop: Pick<StopInfo, 'id' | 'feed' | 'code'>): string {
  return stop.feed === 'gtt' ? stop.code : stop.id
}

/** The same from the full id alone (`gtt:39` → `39`), for a stop no longer in the index. */
export function stopParamFromId(id: string): string {
  return id.startsWith('gtt:') ? id.slice('gtt:'.length) : id
}

/**
 * The server could not be reached at all: no network, or a proxy in front of it answered
 * instead (a 502–504 without the API's error format: Vite in development, Nginx once
 * deployed). Screens then show the last data they kept.
 */
export function isUnreachable(error: unknown): boolean {
  if (!(error instanceof ApiError)) return false
  if (error.code === 'network') return true
  return error.code === 'http_error' && error.status >= 502 && error.status <= 504
}

/** What to tell the user about a failed call, in Italian. */
export function errorText(error: unknown): string {
  if (!(error instanceof ApiError)) return 'Qualcosa non ha funzionato. Riprova.'
  if (error.code !== 'network' && isUnreachable(error)) {
    return 'Server non raggiungibile. Riprova tra poco.'
  }
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
