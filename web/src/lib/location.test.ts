import { describe, expect, it, vi } from 'vitest'
import { LocationError, locate, type Geolocation } from './location'

function fake(answer: { coords?: object; error?: { code: number } }): Geolocation {
  return {
    getCurrentPosition: vi.fn((success, failure) => {
      if (answer.error) failure?.(answer.error as GeolocationPositionError)
      else success({ coords: answer.coords } as GeolocationPosition)
    }),
  }
}

describe('locate', () => {
  it('returns the position', async () => {
    const geolocation = fake({ coords: { latitude: 45.06, longitude: 7.68, accuracy: 25 } })
    await expect(locate(geolocation)).resolves.toEqual({ lat: 45.06, lon: 7.68, accuracy: 25 })
    expect(vi.mocked(geolocation.getCurrentPosition).mock.calls[0][2]).toEqual({
      enableHighAccuracy: true,
      timeout: 10_000,
      maximumAge: 30_000,
    })
  })

  it('explains each failure', async () => {
    for (const [code, reason] of [
      [1, 'denied'],
      [2, 'unavailable'],
      [3, 'timeout'],
    ] as const) {
      const error = await locate(fake({ error: { code } })).catch((e: unknown) => e)
      expect(error).toBeInstanceOf(LocationError)
      expect(error).toMatchObject({ reason })
    }
  })

  it('copes with a browser without geolocation', async () => {
    await expect(locate(null)).rejects.toMatchObject({ reason: 'unsupported' })
  })
})
