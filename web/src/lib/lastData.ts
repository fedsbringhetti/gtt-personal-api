/**
 * The last data each screen received, kept in the browser's storage so the app can show
 * them without network ("Sei offline · dati delle 07:52"). Only what the server already
 * has: favorites and departures, nothing else.
 */

const PREFIX = 'intime:'
/** Stops whose departures are kept; opening an eleventh forgets the least recent. */
const MAX_STOPS = 10

export interface Saved<T> {
  /** When the data was saved, ISO 8601. */
  savedAt: string
  data: T
}

/** The part of `localStorage` used here, so tests can pass a fake one. */
export type Storage = Pick<globalThis.Storage, 'getItem' | 'setItem' | 'removeItem'>

function browserStorage(): Storage | null {
  try {
    return globalThis.localStorage ?? null
  } catch {
    // Some browsers throw on access when storage is disabled (private mode, settings).
    return null
  }
}

export class LastData {
  constructor(
    private readonly storage: Storage | null = browserStorage(),
    private readonly now: () => Date = () => new Date(),
  ) {}

  saveFavorites<T>(data: T) {
    this.write('favorites', data)
  }

  loadFavorites<T>(): Saved<T> | null {
    return this.read('favorites')
  }

  /** `stop` as in the address: `39` or `gtt:39`. */
  saveStop<T>(stop: string, data: T) {
    this.write(`stop:${stop}`, data)
    const recent = [stop, ...this.recentStops().filter((other) => other !== stop)]
    for (const forgotten of recent.slice(MAX_STOPS)) this.remove(`stop:${forgotten}`)
    this.write('stops', recent.slice(0, MAX_STOPS), false)
  }

  loadStop<T>(stop: string): Saved<T> | null {
    return this.read(`stop:${stop}`)
  }

  private recentStops(): string[] {
    const saved = this.read<string[]>('stops')
    return Array.isArray(saved?.data) ? saved.data : []
  }

  private write(key: string, data: unknown, stamped = true) {
    const value: Saved<unknown> = { savedAt: stamped ? this.now().toISOString() : '', data }
    try {
      this.storage?.setItem(PREFIX + key, JSON.stringify(value))
    } catch {
      // Storage full or disabled: the app works anyway, without data for offline use.
    }
  }

  private read<T>(key: string): Saved<T> | null {
    try {
      const text = this.storage?.getItem(PREFIX + key)
      return text ? (JSON.parse(text) as Saved<T>) : null
    } catch {
      return null
    }
  }

  private remove(key: string) {
    try {
      this.storage?.removeItem(PREFIX + key)
    } catch {
      // Nothing to do.
    }
  }
}

export const lastData = new LastData()
