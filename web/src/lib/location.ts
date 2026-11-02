/** The phone's position, asked to the browser only when the near-me screen needs it. */

export interface Position {
  lat: number
  lon: number
  /** Meters: the true position is within this distance. */
  accuracy: number
}

/** Why there is no position, with what to tell the user. */
export class LocationError extends Error {
  readonly reason: 'denied' | 'unavailable' | 'timeout' | 'unsupported'

  constructor(reason: LocationError['reason'], message: string) {
    super(message)
    this.name = 'LocationError'
    this.reason = reason
  }
}

/** Waits at most this long; accepts a position at most this old. */
const TIMEOUT_MS = 10_000
const MAX_AGE_MS = 30_000

/** The part of `navigator.geolocation` used here, so tests can pass a fake one. */
export type Geolocation = Pick<globalThis.Geolocation, 'getCurrentPosition'>

export function locate(
  // `null` (not `undefined`, which would pick the default) stands for a browser without it.
  geolocation: Geolocation | null = globalThis.navigator?.geolocation ?? null,
): Promise<Position> {
  return new Promise((resolve, reject) => {
    if (!geolocation) {
      reject(new LocationError('unsupported', 'Questo browser non sa dire dove ti trovi.'))
      return
    }
    geolocation.getCurrentPosition(
      ({ coords }) =>
        resolve({ lat: coords.latitude, lon: coords.longitude, accuracy: coords.accuracy }),
      (error) => reject(toLocationError(error)),
      { enableHighAccuracy: true, timeout: TIMEOUT_MS, maximumAge: MAX_AGE_MS },
    )
  })
}

// The codes of `GeolocationPositionError`, written out because the class does not exist
// outside browsers (tests run in Node).
const PERMISSION_DENIED = 1
const POSITION_UNAVAILABLE = 2

function toLocationError(error: Pick<GeolocationPositionError, 'code'>): LocationError {
  switch (error.code) {
    case PERMISSION_DENIED:
      return new LocationError('denied', 'Il permesso di usare la posizione è stato negato.')
    case POSITION_UNAVAILABLE:
      return new LocationError('unavailable', 'Non riesco a trovare la tua posizione.')
    default:
      return new LocationError('timeout', 'La posizione non è arrivata in tempo.')
  }
}
