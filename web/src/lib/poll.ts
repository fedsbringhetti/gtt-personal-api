/** The parts of `document` that polling needs, so tests can pass a fake one. */
export interface Visibility {
  readonly visibilityState: DocumentVisibilityState
  addEventListener(type: 'visibilitychange', listener: () => void): void
  removeEventListener(type: 'visibilitychange', listener: () => void): void
}

/**
 * Calls `load` now and every `intervalMs` while the page is visible. In the background
 * (another app, the phone locked) it stops; when the page comes back it loads at once if
 * the last load is older than the interval. When the network comes back (`online` on
 * `network`) it loads at once too. Returns the function that stops it all.
 */
export function startPolling(
  load: () => void,
  intervalMs: number,
  page: Visibility = document,
  now: () => number = Date.now,
  network: Pick<EventTarget, 'addEventListener' | 'removeEventListener'> = window,
): () => void {
  let last = 0
  let timer: ReturnType<typeof setTimeout> | undefined

  const run = () => {
    last = now()
    load()
    schedule(intervalMs)
  }

  const schedule = (delay: number) => {
    clearTimeout(timer)
    timer = setTimeout(() => {
      if (page.visibilityState === 'visible') run()
    }, delay)
  }

  const onVisibilityChange = () => {
    if (page.visibilityState !== 'visible') {
      clearTimeout(timer)
      return
    }
    const elapsed = now() - last
    if (elapsed >= intervalMs) run()
    else schedule(intervalMs - elapsed)
  }

  const onOnline = () => {
    if (page.visibilityState === 'visible') run()
  }

  page.addEventListener('visibilitychange', onVisibilityChange)
  network.addEventListener('online', onOnline)
  run()
  return () => {
    clearTimeout(timer)
    page.removeEventListener('visibilitychange', onVisibilityChange)
    network.removeEventListener('online', onOnline)
  }
}
