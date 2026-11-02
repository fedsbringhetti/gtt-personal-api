import { describe, expect, it } from 'vitest'
import type { Status } from './api'
import { describeStopIndex, describeUpstream } from './status'

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
