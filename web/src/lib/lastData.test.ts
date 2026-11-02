import { describe, expect, it } from 'vitest'
import { LastData, type Storage } from './lastData'

function memory(): Storage & { keys: () => string[] } {
  const items = new Map<string, string>()
  return {
    getItem: (key) => items.get(key) ?? null,
    setItem: (key, value) => void items.set(key, value),
    removeItem: (key) => void items.delete(key),
    keys: () => [...items.keys()].sort(),
  }
}

const at = () => new Date('2026-11-02T07:52:00Z')

describe('LastData', () => {
  it('keeps the last favorites with the time they were saved', () => {
    const last = new LastData(memory(), at)
    expect(last.loadFavorites()).toBeNull()
    last.saveFavorites({ favorites: [1, 2] })
    expect(last.loadFavorites()).toEqual({
      savedAt: '2026-11-02T07:52:00.000Z',
      data: { favorites: [1, 2] },
    })
  })

  it('keeps the departures of the ten most recent stops', () => {
    const storage = memory()
    const last = new LastData(storage, at)
    for (let stop = 1; stop <= 11; stop++) last.saveStop(String(stop), { stop })
    expect(last.loadStop('1')).toBeNull()
    expect(last.loadStop('2')?.data).toEqual({ stop: 2 })
    expect(last.loadStop('11')?.data).toEqual({ stop: 11 })
    // Opening stop 2 again makes it the most recent: stop 3 goes first next time.
    last.saveStop('2', { stop: 2 })
    last.saveStop('12', { stop: 12 })
    expect(last.loadStop('2')).not.toBeNull()
    expect(last.loadStop('3')).toBeNull()
    expect(storage.keys().filter((key) => key.startsWith('intime:stop:'))).toHaveLength(10)
  })

  it('works without storage and with broken entries', () => {
    const none = new LastData(null, at)
    none.saveFavorites({})
    expect(none.loadFavorites()).toBeNull()

    const storage = memory()
    storage.setItem('intime:favorites', '{not json')
    expect(new LastData(storage, at).loadFavorites()).toBeNull()
  })

  it('survives a full storage', () => {
    const full: Storage = {
      getItem: () => null,
      setItem: () => {
        throw new DOMException('Quota exceeded', 'QuotaExceededError')
      },
      removeItem: () => {},
    }
    expect(() => new LastData(full, at).saveStop('39', {})).not.toThrow()
  })
})
