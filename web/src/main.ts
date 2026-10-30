import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'

// Compiled Svelte components are plain JavaScript: `mount` renders one into the page.
mount(App, { target: document.getElementById('app')! })
