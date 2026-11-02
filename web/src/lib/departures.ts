/** Line filters on the stop screen. Line names are compared ignoring case (`9b` = `9B`). */
import type { Departure, LineRef } from './api'

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase()

/**
 * The filter chips: the lines that serve the stop, in the server's order, then any other
 * line that appears in the departures (a diversion, a replacement bus).
 */
export function lineChoices(
  stopLines: Pick<LineRef, 'name' | 'mode'>[],
  departures: Pick<Departure, 'line' | 'mode'>[],
): { name: string; mode: string | null }[] {
  const choices: { name: string; mode: string | null }[] = []
  const add = (name: string, mode: string | null) => {
    if (!choices.some((choice) => same(choice.name, name))) choices.push({ name, mode })
  }
  for (const line of stopLines) add(line.name, line.mode)
  for (const departure of departures) add(departure.line, departure.mode)
  return choices
}

/** The departures of the chosen lines; all of them when none is chosen. */
export function filterDepartures<T extends Pick<Departure, 'line'>>(
  departures: T[],
  chosen: string[],
): T[] {
  if (chosen.length === 0) return departures
  return departures.filter((departure) => chosen.some((line) => same(line, departure.line)))
}

/** Adds the line to the choice, or removes it when it is already there. */
export function toggleLine(chosen: string[], line: string): string[] {
  return chosen.some((name) => same(name, line))
    ? chosen.filter((name) => !same(name, line))
    : [...chosen, line]
}

export function isChosen(chosen: string[], line: string): boolean {
  return chosen.some((name) => same(name, line))
}
