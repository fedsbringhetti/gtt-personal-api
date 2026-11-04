/** What the info screen says about the server, from `GET /api/status`. */
import type { Status } from './api'
import { formatDayTime } from './format'

export interface Line {
  /** Short, in the row: "Funzionano". */
  value: string
  /** Under it: "ultimo contatto oggi alle 15:40". */
  detail: string | null
  tone: 'ok' | 'problem' | 'neutral'
}

/** OpenTripPlanner's real-time data: whether the upstream answers. */
export function describeUpstream(status: Status, now?: Date): Line {
  return describeHealth(status.upstream, now)
}

/** GTT's predictions: whether the feed answers, and how fresh its newest data is. */
export function describeGtt(status: Status, now?: Date): Line {
  if (!status.gtt) {
    return { value: 'Spente', detail: 'solo orari di muoversiatorino.it', tone: 'neutral' }
  }
  const line = describeHealth(status.gtt.trip_updates, now)
  const newest = status.gtt.feed?.newest_update
  if (line.tone === 'ok' && newest) line.detail = `ultimo dato ${formatDayTime(newest, now, true)}`
  return line
}

/** Today's difference between the two, and GTT's predictions set aside as implausible.
 * `null` when GTT is disabled. Short, to fit a 375 px row. */
export function describeComparison(status: Status): Line | null {
  if (!status.gtt) return null
  const { used, mismatches, median_difference_seconds: median } = status.gtt.comparison
  const total = used + mismatches
  let value = 'Nessun dato ancora'
  if (median !== null) value = median < 60 ? 'meno di 1 min' : `${Math.round(median / 60)} min`
  let detail: string | null = null
  if (total > 0) {
    const of = `su ${count(total)}`
    if (mismatches === 0) detail = `nessuna scartata ${of}`
    else if (mismatches === 1) detail = `1 scartata ${of}`
    else detail = `${count(mismatches)} scartate ${of}`
  }
  return { value, detail, tone: 'neutral' }
}

function describeHealth(health: Status['upstream'], now?: Date): Line {
  const { ok, last_success, last_failure } = health
  if (ok === null) return { value: 'Nessun contatto ancora', detail: null, tone: 'neutral' }
  if (ok) {
    return {
      value: 'Funzionano',
      detail: last_success ? `ultimo contatto ${formatDayTime(last_success, now, true)}` : null,
      tone: 'ok',
    }
  }
  return {
    value: 'Non rispondono',
    detail: last_failure ? `ultimo errore ${formatDayTime(last_failure, now, true)}` : null,
    tone: 'problem',
  }
}

/** The local list of stops used by search and near me. */
export function describeStopIndex(status: Status, now?: Date): Line {
  if (!status.stop_index) {
    return { value: 'Non ancora scaricato', detail: 'si scarica alla prima accensione', tone: 'problem' }
  }
  return {
    value: `${count(status.stop_index.count)} fermate`,
    detail: `aggiornato ${formatDayTime(status.stop_index.synced_at, now)}`,
    tone: 'neutral',
  }
}

/** Italian grouping, also below five digits ("7.058", not "7058"), for a steady layout. */
function count(n: number): string {
  return n.toLocaleString('it-IT', { useGrouping: 'always' })
}
