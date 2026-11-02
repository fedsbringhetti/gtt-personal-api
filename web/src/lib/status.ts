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

/** The real-time data: whether the upstream answers. */
export function describeUpstream(status: Status, now?: Date): Line {
  const { ok, last_success, last_failure } = status.upstream
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
  // Italian rules group only from five digits ("7058"): force "7.058" for a steady layout.
  const count = status.stop_index.count.toLocaleString('it-IT', { useGrouping: 'always' })
  return {
    value: `${count} fermate`,
    detail: `aggiornato ${formatDayTime(status.stop_index.synced_at, now)}`,
    tone: 'neutral',
  }
}
