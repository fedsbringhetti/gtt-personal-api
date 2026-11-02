import { describe, expect, it } from 'vitest'
import { matchingFavorite, sameLines, stopIdOf } from './favorites'

describe('stopIdOf', () => {
  it('adds the GTT feed to a bare code', () => {
    expect(stopIdOf('39')).toBe('gtt:39')
    expect(stopIdOf('2:S00219')).toBe('2:S00219')
  })
})

describe('sameLines', () => {
  it('ignores order, case and repeats', () => {
    expect(sameLines(['4', '15'], ['15', '4'])).toBe(true)
    expect(sameLines(['9b'], ['9B', '9b'])).toBe(true)
    expect(sameLines([], [])).toBe(true)
    expect(sameLines(['4'], [])).toBe(false)
    expect(sameLines(['4', '15'], ['4', '16'])).toBe(false)
  })
})

describe('matchingFavorite', () => {
  const favorites = [
    { id: 1, stop_id: 'gtt:39', lines: [] },
    { id: 2, stop_id: 'gtt:39', lines: ['4'] },
    { id: 3, stop_id: 'gtt:40', lines: [] },
  ]

  it('needs the same stop and the same lines', () => {
    expect(matchingFavorite(favorites, 'gtt:39', [])?.id).toBe(1)
    expect(matchingFavorite(favorites, 'gtt:39', ['4'])?.id).toBe(2)
    expect(matchingFavorite(favorites, 'gtt:39', ['4', '15'])).toBeUndefined()
    expect(matchingFavorite(favorites, 'gtt:41', [])).toBeUndefined()
  })
})
