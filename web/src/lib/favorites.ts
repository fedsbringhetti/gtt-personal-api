/** Favorites seen from the stop screen and the favorites screen. */
import type { Favorite } from './api'

/** The full id of a stop as written in the address: `39` → `gtt:39`, `2:S00219` as it is. */
export function stopIdOf(param: string): string {
  return param.includes(':') ? param : `gtt:${param}`
}

/** The same lines in any order, ignoring case. */
export function sameLines(a: string[], b: string[]): boolean {
  const normalize = (lines: string[]) => [...new Set(lines.map((line) => line.toLowerCase()))].sort()
  const [left, right] = [normalize(a), normalize(b)]
  return left.length === right.length && left.every((line, index) => line === right[index])
}

/**
 * The favorite for this stop with exactly these lines, if any: what the star on the stop
 * screen shows. A stop can have several favorites with different lines ("Casa, solo 4").
 */
export function matchingFavorite(
  favorites: Pick<Favorite, 'id' | 'stop_id' | 'lines'>[],
  stopId: string,
  lines: string[],
) {
  return favorites.find(
    (favorite) => favorite.stop_id === stopId && sameLines(favorite.lines, lines),
  )
}
