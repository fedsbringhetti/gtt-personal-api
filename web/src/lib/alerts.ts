/** GTT's alerts on the stop screen: what the closed banner says, which alerts and lines. */
import type { Alert } from './api'

/** "1 avviso GTT", "3 avvisi GTT". */
export function alertsCount(alerts: Alert[]): string {
  return alerts.length === 1 ? '1 avviso GTT' : `${alerts.length} avvisi GTT`
}

/** The titles in one line, for the closed banner. */
export function alertsTitles(alerts: Alert[]): string {
  return alerts.map((alert) => alert.title).join(' · ')
}

/**
 * The alerts that matter for the lines on screen: those naming one of them, and those naming
 * no line (they concern the whole stop). With no filter, all of them. A "stop suspended" alert
 * names every line serving the stop, so it stays whatever the filter.
 */
export function alertsForLines(alerts: Alert[], lines: string[]): Alert[] {
  if (lines.length === 0) return alerts
  const chosen = new Set(lines.map((line) => line.toLowerCase()))
  return alerts.filter(
    (alert) =>
      alert.lines.length === 0 || alert.lines.some((line) => chosen.has(line.name.toLowerCase())),
  )
}

/** Names of the lines named by the alerts, lowercase, to mark their departures. */
export function linesWithAlerts(alerts: Alert[]): Set<string> {
  return new Set(alerts.flatMap((alert) => alert.lines.map((line) => line.name.toLowerCase())))
}

/** "Dettagli su gtt.to.it": the site the link opens, without `www.`. */
export function linkLabel(url: string): string {
  try {
    return `Dettagli su ${new URL(url).hostname.replace(/^www\./, '')}`
  } catch {
    return 'Dettagli'
  }
}
