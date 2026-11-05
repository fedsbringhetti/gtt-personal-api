/// <reference lib="webworker" />
// The service worker: a script the browser runs apart from the page, between the app and the
// network. It keeps the app's own files on the phone (the build lists them in
// `self.__WB_MANIFEST`), so the app opens without network. API calls are not touched: they
// always go to the server.
import { cleanupOutdatedCaches, createHandlerBoundToURL, precacheAndRoute } from 'workbox-precaching'
import { NavigationRoute, registerRoute } from 'workbox-routing'
import { readPayload } from './lib/pushPayload'

declare let self: ServiceWorkerGlobalScope

precacheAndRoute(self.__WB_MANIFEST)
cleanupOutdatedCaches()

// Every screen is `index.html` (the app has its own router), so opening `/fermata/39` without
// network gets the cached page. `/api/` stays out: the server answers it.
registerRoute(new NavigationRoute(createHandlerBoundToURL('/index.html'), { denylist: [/^\/api\//] }))

// A new version waits until the page asks for it ("Aggiorna"), then takes over.
self.addEventListener('message', (event) => {
  if (event.data?.type === 'SKIP_WAITING') self.skipWaiting()
})

// Web Push. The payload is read by `readPayload` (pure, tested); see `lib/pushPayload.ts`.

self.addEventListener('push', (event) => {
  const { title, body, url, tag } = readPayload(() => event.data?.json())
  event.waitUntil(
    self.registration.showNotification(title, {
      body,
      tag,
      // A replaced notification (same tag) is silent unless it is asked to alert again.
      ...(tag ? { renotify: true } : {}),
      data: { url },
      icon: '/pwa-192x192.png',
      lang: 'it',
    }),
  )
})

// A tap opens the app on the notification's screen: an open window is reused, so the user
// does not end up with two copies of the app.
self.addEventListener('notificationclick', (event) => {
  event.notification.close()
  const url = new URL(event.notification.data?.url ?? '/', self.location.origin)
  // The payload comes from our own server, but a link out of this origin is never followed.
  if (url.origin !== self.location.origin) return
  event.waitUntil(
    (async () => {
      try {
        const sameOrigin = (client: WindowClient) => new URL(client.url).origin === self.location.origin
        // Prefer a window this worker controls (the app itself); then any window of the origin.
        const controlled = (await self.clients.matchAll({ type: 'window' })).filter(sameOrigin)
        const open =
          controlled[0] ??
          (await self.clients.matchAll({ type: 'window', includeUncontrolled: true })).find(sameOrigin)
        if (open) {
          try {
            await open.focus()
            // `navigate` reloads the page at `url`; it is missing on some older browsers.
            if (!('navigate' in open)) throw new Error('navigate is missing')
            await open.navigate(url.href)
            return
          } catch {
            // Fall through to a new window.
          }
        }
        await self.clients.openWindow(url.href)
      } catch {
        // Nothing left to try; never reject the event.
      }
    })(),
  )
})
