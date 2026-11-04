import { describe, expect, it } from 'vitest'
import type { Status } from './api'
import { describeComparison, describeGtt, describeStopIndex, describeUpstream } from './status'

const now = new Date('2026-11-02T15:00:00+01:00')

const status: Status = {
  version: '0.2.0',
  time: '2026-11-02T15:00:00+01:00',
  upstream: {
    ok: true,
    last_success: '2026-11-02T14:40:00Z',
    last_failure: null,
    last_error: null,
    consecutive_failures: 0,
  },
  stop_index: { synced_at: '2026-11-02T03:00:00Z', count: 7058 },
  gtt: null,
}

const withGtt: Status = {
  ...status,
  gtt: {
    trip_updates: { ...status.upstream, last_success: '2026-11-02T14:40:10Z' },
    feed: { runs: 383, newest_update: '2026-11-02T14:39:58Z' },
    comparison: {
      date: '2026-11-02',
      used: 1236,
      mismatches: 4,
      compared: 980,
      median_difference_seconds: 125,
    },
  },
}

/** `withGtt` with some fields of the comparison changed. */
function comparison(changes: Partial<NonNullable<Status['gtt']>['comparison']>): Status {
  const gtt = withGtt.gtt!
  return { ...withGtt, gtt: { ...gtt, comparison: { ...gtt.comparison, ...changes } } }
}

describe('describeUpstream', () => {
  it('says when the real-time data works', () => {
    expect(describeUpstream(status, now)).toEqual({
      value: 'Funzionano',
      detail: 'ultimo contatto 15:40',
      tone: 'ok',
    })
  })

  it('says when it fails, and when it was never called', () => {
    const failing = {
      ...status,
      upstream: { ...status.upstream, ok: false, last_failure: '2026-11-01T22:10:00Z' },
    }
    expect(describeUpstream(failing, now)).toMatchObject({
      value: 'Non rispondono',
      detail: 'ultimo errore ieri alle 23:10',
      tone: 'problem',
    })
    const fresh = { ...status, upstream: { ...status.upstream, ok: null } }
    expect(describeUpstream(fresh, now).value).toBe('Nessun contatto ancora')
  })
})

describe('describeStopIndex', () => {
  it('shows the count with Italian grouping and when it was updated', () => {
    expect(describeStopIndex(status, now)).toEqual({
      value: '7.058 fermate',
      detail: 'aggiornato oggi alle 04:00',
      tone: 'neutral',
    })
  })

  it('says when the list is not there yet', () => {
    expect(describeStopIndex({ ...status, stop_index: null }, now).value).toBe(
      'Non ancora scaricato',
    )
  })
})

describe('describeGtt', () => {
  it('shows the time of the newest prediction', () => {
    expect(describeGtt(withGtt, now)).toEqual({
      value: 'Funzionano',
      detail: 'ultimo dato 15:39',
      tone: 'ok',
    })
  })

  it('says when GTT fails, and when the server has it disabled', () => {
    const gtt = withGtt.gtt!
    const trip_updates = { ...gtt.trip_updates, ok: false, last_failure: '2026-11-02T14:58:00Z' }
    expect(describeGtt({ ...withGtt, gtt: { ...gtt, trip_updates } }, now)).toMatchObject({
      value: 'Non rispondono',
      detail: 'ultimo errore 15:58',
      tone: 'problem',
    })
    expect(describeGtt(status, now)).toMatchObject({ value: 'Spente', tone: 'neutral' })
  })
})

describe('describeComparison', () => {
  it('shows the median difference in minutes and the predictions set aside', () => {
    expect(describeComparison(withGtt)).toEqual({
      value: '2 min',
      detail: '4 scartate su 1.240',
      tone: 'neutral',
    })
  })

  it('handles small numbers and an empty day', () => {
    expect(
      describeComparison(comparison({ median_difference_seconds: 40, mismatches: 0 })),
    ).toMatchObject({
      value: 'meno di 1 min',
      detail: 'nessuna scartata su 1.236',
    })
    expect(describeComparison(comparison({ mismatches: 1, used: 9 }))?.detail).toBe(
      '1 scartata su 10',
    )
    const empty = { used: 0, mismatches: 0, compared: 0, median_difference_seconds: null }
    expect(describeComparison(comparison(empty))).toEqual({
      value: 'Nessun dato ancora',
      detail: null,
      tone: 'neutral',
    })
    expect(describeComparison(status)).toBeNull()
  })
})
