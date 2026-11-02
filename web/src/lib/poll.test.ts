import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { startPolling, type Visibility } from './poll'

class FakePage extends EventTarget implements Visibility {
  visibilityState: DocumentVisibilityState = 'visible'

  set(state: DocumentVisibilityState) {
    this.visibilityState = state
    this.dispatchEvent(new Event('visibilitychange'))
  }
}

describe('startPolling', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('loads now and then at every interval', () => {
    const load = vi.fn()
    const stop = startPolling(load, 30_000, new FakePage(), Date.now, new EventTarget())
    expect(load).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(30_000)
    expect(load).toHaveBeenCalledTimes(2)
    vi.advanceTimersByTime(60_000)
    expect(load).toHaveBeenCalledTimes(4)
    stop()
    vi.advanceTimersByTime(60_000)
    expect(load).toHaveBeenCalledTimes(4)
  })

  it('pauses in the background and catches up when visible again', () => {
    const page = new FakePage()
    const load = vi.fn()
    startPolling(load, 30_000, page, Date.now, new EventTarget())
    page.set('hidden')
    vi.advanceTimersByTime(5 * 60_000)
    expect(load).toHaveBeenCalledTimes(1)
    page.set('visible')
    expect(load).toHaveBeenCalledTimes(2)
  })

  it('loads at once when the network comes back', () => {
    const network = new EventTarget()
    const load = vi.fn()
    startPolling(load, 30_000, new FakePage(), Date.now, network)
    vi.advanceTimersByTime(5_000)
    network.dispatchEvent(new Event('online'))
    expect(load).toHaveBeenCalledTimes(2)
  })

  it('does not load twice when it comes back quickly', () => {
    const page = new FakePage()
    const load = vi.fn()
    startPolling(load, 30_000, page, Date.now, new EventTarget())
    vi.advanceTimersByTime(10_000)
    page.set('hidden')
    page.set('visible')
    expect(load).toHaveBeenCalledTimes(1)
    vi.advanceTimersByTime(20_000)
    expect(load).toHaveBeenCalledTimes(2)
  })
})
