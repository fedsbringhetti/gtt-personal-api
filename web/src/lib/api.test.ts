import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  ApiError,
  addFavorite,
  errorText,
  getDepartures,
  getStop,
  nearbyStops,
  removeFavorite,
  searchStops,
  stopParam,
  stopParamFromId,
  isUnreachable,
  updateFavorite,
} from './api'

function respond(status: number, body: unknown) {
  // A 204 response must have no body at all, not even an empty one.
  const text = status === 204 ? null : typeof body === 'string' ? body : JSON.stringify(body)
  const fetch = vi.fn().mockResolvedValue(new Response(text, { status }))
  vi.stubGlobal('fetch', fetch)
  return fetch
}

afterEach(() => {
  vi.unstubAllGlobals()
})

describe('requests', () => {
  it('encodes the search query', async () => {
    const fetch = respond(200, { stops: [] })
    await searchStops("c.so sant'anna & co")
    expect(fetch.mock.calls[0][0]).toBe("/api/stops?q=c.so+sant%27anna+%26+co&limit=20")
  })

  it('sends the position and radius of a nearby search', async () => {
    const fetch = respond(200, { stops: [] })
    await nearbyStops({ lat: 45.0626, lon: 7.679 }, 500)
    expect(fetch.mock.calls[0][0]).toBe('/api/stops/nearby?lat=45.0626&lon=7.679&radius=500')
  })

  it('encodes a full stop id in the path', async () => {
    const fetch = respond(200, { departures: [] })
    await getDepartures('2:S00219')
    expect(fetch.mock.calls[0][0]).toBe('/api/stops/2%3AS00219/departures?minutes=60&limit=30')
  })
})

describe('favorites', () => {
  it('sends changes as JSON', async () => {
    const fetch = respond(201, { id: 1 })
    await addFavorite('39', ['4', '15'])
    const [path, init] = fetch.mock.calls[0]
    expect(path).toBe('/api/favorites')
    expect(init).toMatchObject({ method: 'POST', headers: { 'Content-Type': 'application/json' } })
    expect(JSON.parse(init.body)).toEqual({ stop: '39', lines: ['4', '15'] })
  })

  it('sends null to clear a label', async () => {
    const fetch = respond(200, { id: 1 })
    await updateFavorite(1, { label: null })
    expect(fetch.mock.calls[0][1]).toMatchObject({ method: 'PATCH', body: '{"label":null}' })
  })

  it('accepts an empty answer to a delete', async () => {
    respond(204, '')
    await expect(removeFavorite(1)).resolves.toBeUndefined()
  })
})

describe('errors', () => {
  it('reads the API error code and message', async () => {
    respond(404, { error: { code: 'stop_not_found', message: 'Stop gtt:999999 not found' } })
    const error = await getStop('999999').catch((e: unknown) => e)
    expect(error).toBeInstanceOf(ApiError)
    expect(error).toMatchObject({ status: 404, code: 'stop_not_found' })
  })

  it('copes with a body that is not the API format', async () => {
    respond(502, '<html>Bad gateway</html>')
    await expect(getStop('39')).rejects.toMatchObject({ status: 502, code: 'http_error' })
  })

  it('reports an unreachable server as a network error', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('Failed to fetch')))
    await expect(getStop('39')).rejects.toMatchObject({ status: 0, code: 'network' })
  })

  it('lets an aborted call through as it is', async () => {
    const abort = new DOMException('Aborted', 'AbortError')
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(abort))
    const controller = new AbortController()
    controller.abort()
    await expect(searchStops('porta', controller.signal)).rejects.toBe(abort)
  })
})

describe('stopParam', () => {
  it('uses the bare code for GTT and the full id otherwise', () => {
    expect(stopParam({ id: 'gtt:39', feed: 'gtt', code: '39' })).toBe('39')
    expect(stopParam({ id: '2:S00219', feed: '2', code: 'S00219' })).toBe('2:S00219')
    expect(stopParamFromId('gtt:39')).toBe('39')
    expect(stopParamFromId('2:S00219')).toBe('2:S00219')
  })
})

describe('isUnreachable', () => {
  it('is true without network or behind a failing proxy', () => {
    expect(isUnreachable(new ApiError(0, 'network', ''))).toBe(true)
    expect(isUnreachable(new ApiError(502, 'http_error', ''))).toBe(true)
    expect(isUnreachable(new ApiError(504, 'http_error', ''))).toBe(true)
  })

  it('is false when the server itself answered', () => {
    expect(isUnreachable(new ApiError(502, 'upstream_error', ''))).toBe(false)
    expect(isUnreachable(new ApiError(500, 'http_error', ''))).toBe(false)
    expect(isUnreachable(new Error('boom'))).toBe(false)
  })
})

describe('errorText', () => {
  it('explains the errors the screens can meet', () => {
    expect(errorText(new ApiError(0, 'network', ''))).toMatch(/connessione/)
    expect(errorText(new ApiError(503, 'index_not_ready', ''))).toMatch(/si sta scaricando/)
    expect(errorText(new ApiError(500, 'internal_error', ''))).toBe(
      'Qualcosa non ha funzionato (500). Riprova.',
    )
    expect(errorText(new Error('boom'))).toBe('Qualcosa non ha funzionato. Riprova.')
  })

  it('reads a proxy failure as an unreachable server', () => {
    expect(errorText(new ApiError(502, 'http_error', ''))).toBe(
      'Server non raggiungibile. Riprova tra poco.',
    )
    expect(errorText(new ApiError(502, 'upstream_error', ''))).toMatch(/orari non risponde/)
  })
})
