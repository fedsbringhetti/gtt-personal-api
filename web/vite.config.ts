/// <reference types="vitest/config" />
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig } from 'vite'
import { VitePWA } from 'vite-plugin-pwa'

export default defineConfig({
  plugins: [
    svelte(),
    // The installable app: a web manifest and a service worker (src/sw.ts) that keeps the
    // app's files on the phone, so it opens without network.
    VitePWA({
      // Our own service worker, so Phase 3 can add Web Push to it.
      strategies: 'injectManifest',
      srcDir: 'src',
      filename: 'sw.ts',
      // A new version waits for "Aggiorna" (src/lib/update.svelte.ts).
      registerType: 'prompt',
      injectRegister: false,
      manifest: {
        name: 'InTime',
        short_name: 'InTime',
        description: 'Partenze in tempo reale dei mezzi di Torino',
        lang: 'it',
        start_url: '/',
        scope: '/',
        display: 'standalone',
        background_color: '#f2f2f7',
        theme_color: '#d62828',
        icons: [
          { src: 'pwa-64x64.png', sizes: '64x64', type: 'image/png' },
          { src: 'pwa-192x192.png', sizes: '192x192', type: 'image/png' },
          { src: 'pwa-512x512.png', sizes: '512x512', type: 'image/png' },
          {
            src: 'maskable-icon-512x512.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'maskable',
          },
        ],
      },
      injectManifest: {
        globPatterns: ['**/*.{js,css,html,svg,png,ico}'],
      },
    }),
  ],
  server: {
    // `npm run dev` serves the interface; API calls go to the backend started with `cargo run`.
    proxy: {
      '/api': 'http://localhost:8080',
    },
  },
  test: {
    include: ['src/**/*.test.ts'],
  },
})
