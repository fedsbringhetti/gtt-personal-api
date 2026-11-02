/**
 * New versions of the app. The service worker downloads a new version in the background
 * when the server has one; it then waits, and `update.available` shows "Aggiorna".
 */
import { registerSW } from 'virtual:pwa-register'

export const update = $state({ available: false })

/** An installed app can stay open for days: look again when it comes back, at most this often. */
const CHECK_EVERY_MS = 60_000

let reloadWithUpdate: ((reload?: boolean) => Promise<void>) | undefined

export function startUpdates() {
  if (!('serviceWorker' in navigator)) return
  reloadWithUpdate = registerSW({
    onNeedRefresh() {
      update.available = true
    },
    onRegisteredSW(_url, registration) {
      if (!registration) return
      let lastCheck = Date.now()
      document.addEventListener('visibilitychange', () => {
        if (document.visibilityState !== 'visible' || Date.now() - lastCheck < CHECK_EVERY_MS) return
        lastCheck = Date.now()
        registration.update()
      })
    },
  })
}

/** "Aggiorna": the new version takes over and the page reloads with it. */
export function applyUpdate() {
  reloadWithUpdate?.(true)
}
