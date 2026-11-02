/// <reference lib="webworker" />
// The service worker: a script the browser runs apart from the page, between the app and the
// network. It keeps the app's own files on the phone (the build lists them in
// `self.__WB_MANIFEST`), so the app opens without network. API calls are not touched: they
// always go to the server.
import { cleanupOutdatedCaches, createHandlerBoundToURL, precacheAndRoute } from 'workbox-precaching'
import { NavigationRoute, registerRoute } from 'workbox-routing'

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
