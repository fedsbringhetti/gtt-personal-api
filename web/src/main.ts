import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { startUpdates } from './lib/update.svelte'

// Compiled Svelte components are plain JavaScript: `mount` renders one into the page.
mount(App, { target: document.getElementById('app')! })

// The service worker: the app opens without network, and new versions are offered.
startUpdates()
