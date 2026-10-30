import { describe, expect, it } from 'vitest'
import { describeStatus, type Status } from './status'

const status: Status = {
  version: '0.1.0',
  time: '2026-10-30T08:00:00+01:00',
  upstream: {
    ok: true,
    last_success: '2026-10-30T07:59:00Z',
    last_failure: null,
    last_error: null,
    consecutive_failures: 0,
  },
  stop_index: { synced_at: '2026-10-30T03:30:00Z', count: 7058 },
}

describe('describeStatus', () => {
  it('shows the version and the stop count with Italian grouping', () => {
    expect(describeStatus(status)).toBe('Server 0.1.0 · 7.058 fermate')
  })

  it('says when the stop index is not there yet', () => {
    expect(describeStatus({ ...status, stop_index: null })).toBe(
      'Server 0.1.0 · fermate non ancora scaricate',
    )
  })
})
