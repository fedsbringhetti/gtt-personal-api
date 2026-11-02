<script lang="ts">
  import { onMount } from 'svelte'
  import { errorText, searchStops, stopParam, type StopInfo } from '../lib/api'
  import BusLoader from '../lib/components/BusLoader.svelte'
  import ErrorCard from '../lib/components/ErrorCard.svelte'
  import Highlighted from '../lib/components/Highlighted.svelte'
  import Icon from '../lib/components/Icon.svelte'
  import LineBadge from '../lib/components/LineBadge.svelte'
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import { displayName } from '../lib/format'
  import { navigate } from '../lib/navigation.svelte'
  import { pathOf } from '../lib/router'

  interface Props {
    /** The query in the address, `?q=...`: what the results are for. */
    query: string
  }

  let { query }: Props = $props()

  /** Pause after the last key before searching, so each letter does not start a call. */
  const PAUSE_MS = 250

  // What is in the field. The address follows it after the pause; when the address changes
  // from outside (the back button), the field follows the address.
  let text = $state('')
  $effect(() => {
    text = query
  })

  let input: HTMLInputElement
  let pending: ReturnType<typeof setTimeout> | undefined

  function showResultsFor(value: string) {
    clearTimeout(pending)
    // `replace`: typing does not fill the history; back leaves the search screen.
    navigate(pathOf({ name: 'search', query: value }), { replace: true })
  }

  function onInput() {
    clearTimeout(pending)
    pending = setTimeout(() => showResultsFor(text), PAUSE_MS)
  }

  function onSubmit(event: SubmitEvent) {
    event.preventDefault()
    showResultsFor(text)
    input.blur() // closes the phone's keyboard, so the results are visible
  }

  function clear() {
    text = ''
    showResultsFor('')
    input.focus()
  }

  onMount(() => {
    if (!query) input.focus()
    return () => clearTimeout(pending)
  })

  let results = $state<{ query: string; stops: StopInfo[] } | null>(null)
  let loading = $state(false)
  let error = $state<string | null>(null)
  let attempt = $state(0)

  // Runs again whenever `query` (or `attempt`, for "Riprova") changes. Returning a function
  // aborts the call still in flight, so an old answer never replaces a newer one.
  $effect(() => {
    const searched = query.trim()
    void attempt
    error = null
    if (searched === '') {
      results = null
      loading = false
      return
    }
    const controller = new AbortController()
    loading = true
    searchStops(searched, controller.signal)
      .then((stops) => {
        results = { query: searched, stops }
        loading = false
      })
      .catch((failure: unknown) => {
        if (controller.signal.aborted) return
        error = errorText(failure)
        loading = false
      })
    return () => controller.abort()
  })
</script>

<ScreenHeader title="Cerca" />

<form role="search" onsubmit={onSubmit}>
  <label class="field">
    <Icon name="search" size={20} />
    <span class="visually-hidden">Fermata, via o codice</span>
    <input
      bind:this={input}
      bind:value={text}
      oninput={onInput}
      type="search"
      placeholder="Fermata, via o codice"
      enterkeyhint="search"
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
    />
  </label>
  {#if text}
    <button type="button" class="cancel" onclick={clear}>Annulla</button>
  {/if}
</form>

{#if error}
  <ErrorCard message={error} retry={() => attempt++} />
{:else if results === null && !loading}
  <p class="card hint">
    Cerca per nome (<i>porta nuova</i>), via (<i>c.so vittorio</i>) o codice della fermata
    (<i>39</i>).
  </p>
{:else if results === null}
  <div class="card"><BusLoader label="Cerco…" /></div>
{:else if results.stops.length === 0}
  <p class="card hint">
    Nessuna fermata per «{results.query}». Prova con una parte del nome o con il codice.
  </p>
{:else}
  <p class="count">
    {results.stops.length === 1 ? '1 fermata' : `${results.stops.length} fermate`}
  </p>
  <ul class="card results" class:loading aria-busy={loading}>
    {#each results.stops as stop (stop.id)}
      {@const name = displayName(stop.name)}
      {@const description = stop.description ? displayName(stop.description) : null}
      <li>
        <a href={pathOf({ name: 'stop', stop: stopParam(stop), lines: [] })}>
          <span class="title">
            <span class="name"><Highlighted text={name} query={results.query} /></span>
            <span class="code">{stop.code}</span>
          </span>
          {#if description && description !== name}
            <span class="description"><Highlighted text={description} query={results.query} /></span>
          {/if}
          {#if stop.lines.length}
            <span class="lines">
              {#each stop.lines as line (line.id)}
                <LineBadge name={line.name} mode={line.mode} size="small" />
              {/each}
            </span>
          {/if}
        </a>
      </li>
    {/each}
  </ul>
{/if}

<style>
  form {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .field {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    height: var(--touch);
    padding: 0 12px;
    border-radius: var(--radius-field);
    background: var(--fill);
    color: var(--text-2);
  }

  input {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    outline: none;
  }

  /* Safari draws its own clear button inside search fields; "Annulla" does that job. */
  input::-webkit-search-cancel-button {
    display: none;
  }

  .cancel {
    height: var(--touch);
    padding: 0 2px;
    border: 0;
    background: transparent;
    color: var(--accent);
  }

  .hint {
    padding: 16px var(--gutter);
    font-size: var(--size-secondary);
    color: var(--text-2);
  }

  .count {
    margin: 0 4px;
    font-size: var(--size-caption);
    color: var(--text-2);
  }

  .results.loading {
    opacity: 0.6;
  }

  li + li {
    border-top: 1px solid var(--separator);
  }

  a {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 0;
  }

  .title {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }

  .name {
    font-size: var(--size-body);
  }

  .code {
    font-size: var(--size-secondary);
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }

  .description {
    font-size: 14px;
    color: var(--text-2);
  }

  .lines {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
</style>
