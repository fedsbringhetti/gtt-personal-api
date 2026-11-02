<script lang="ts">
  import { onMount } from 'svelte'
  import { errorText, nearbyStops, stopParam, type NearbyStop } from '../lib/api'
  import BusLoader from '../lib/components/BusLoader.svelte'
  import ErrorCard from '../lib/components/ErrorCard.svelte'
  import Icon from '../lib/components/Icon.svelte'
  import LineBadge from '../lib/components/LineBadge.svelte'
  import Notice from '../lib/components/Notice.svelte'
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import { displayName, formatDistance } from '../lib/format'
  import { LocationError, locate } from '../lib/location'
  import { pathOf } from '../lib/router'

  /** First search radius, as in the mockups; the wider one is tried when it finds nothing. */
  const NEAR_M = 500
  const WIDE_M = 1500
  /** Above this, the position is rough enough to say so. */
  const ROUGH_ACCURACY_M = 200

  type Result = { stops: NearbyStop[]; radius: number; accuracy: number }

  let phase = $state<'locating' | 'loading' | 'done' | 'failed'>('locating')
  let result = $state<Result | null>(null)
  let failure = $state<{ denied: boolean; message: string } | null>(null)
  let call: AbortController | undefined

  // The position lives only inside this function: it is not kept, not put in the address
  // and only sent to our server for this one search.
  async function search() {
    call?.abort()
    const current = new AbortController()
    call = current
    phase = 'locating'
    failure = null
    try {
      const position = await locate()
      if (current.signal.aborted) return
      phase = 'loading'
      let radius = NEAR_M
      let stops = await nearbyStops(position, radius, current.signal)
      if (stops.length === 0) {
        radius = WIDE_M
        stops = await nearbyStops(position, radius, current.signal)
      }
      result = { stops, radius, accuracy: position.accuracy }
      phase = 'done'
    } catch (error) {
      if (current.signal.aborted) return
      failure =
        error instanceof LocationError
          ? { denied: error.reason === 'denied', message: error.message }
          : { denied: false, message: errorText(error) }
      phase = 'failed'
    }
  }

  // Opening this screen is the tap that asks for the position (docs/plan/phase-2.md, Q).
  onMount(() => {
    search()
    return () => call?.abort()
  })

  let busy = $derived(phase === 'locating' || phase === 'loading')
</script>

<ScreenHeader title="Vicino a me" />

<div class="bar">
  <p>
    {result ? `Entro ${formatDistance(result.radius)}` : 'Fermate entro 500 m'} · la posizione
    serve solo per questa ricerca e non viene salvata
  </p>
  <button type="button" class="refresh" disabled={busy} onclick={search}>
    <Icon name="refresh" size={16} stroke={2.4} />Aggiorna
  </button>
</div>

{#if failure?.denied}
  <div class="card help" role="alert">
    <p><strong>{failure.message}</strong></p>
    <p>Per riattivarlo:</p>
    <ul>
      <li>
        <b>iPhone</b>: Impostazioni › Privacy e sicurezza › Localizzazione › Safari (o GTT, se
        l'hai aggiunta alla schermata Home) › «Mentre usi l'app».
      </li>
      <li>
        <b>Android</b>: tocca il lucchetto accanto all'indirizzo › Autorizzazioni › Posizione.
      </li>
    </ul>
    <p>Poi tocca «Aggiorna». Oppure <a href="/cerca">cerca la fermata per nome</a>.</p>
  </div>
{:else if failure && !result}
  <ErrorCard message={failure.message} retry={search} />
{:else if !result}
  <div class="card">
    <BusLoader label={phase === 'locating' ? 'Cerco la tua posizione…' : 'Cerco le fermate…'} />
  </div>
{/if}

{#if result && !failure?.denied}
  {#if failure}
    <Notice message="Fermate non aggiornate. {failure.message}" retry={search} />
  {/if}
  {#if busy}
    <p class="status" role="status">
      {phase === 'locating' ? 'Aggiorno la posizione…' : 'Cerco le fermate…'}
    </p>
  {/if}
  {#if result.accuracy > ROUGH_ACCURACY_M}
    <p class="status">Posizione approssimativa (±{formatDistance(result.accuracy)}).</p>
  {/if}
  {#if result.stops.length === 0}
    <p class="card empty">
      Nessuna fermata entro {formatDistance(result.radius)}. Prova a
      <a href="/cerca">cercarla per nome</a>.
    </p>
  {:else}
    {#if result.radius !== NEAR_M}
      <p class="status">
        Nessuna fermata entro {formatDistance(NEAR_M)}: ecco quelle entro
        {formatDistance(result.radius)}.
      </p>
    {/if}
    <ul class="card stops" class:busy>
      {#each result.stops as stop (stop.id)}
        <li>
          <a href={pathOf({ name: 'stop', stop: stopParam(stop), lines: [] })}>
            <span class="name">
              {displayName(stop.name)} <span class="code">· {stop.code}</span>
            </span>
            <span class="distance">{formatDistance(stop.distance_m)}</span>
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
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .bar p {
    flex: 1;
    margin: 0;
    font-size: 14px;
    color: var(--text-2);
  }

  .refresh {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 36px;
    padding: 0 12px;
    border: 0;
    border-radius: 18px;
    background: var(--surface);
    color: var(--accent);
    font-size: var(--size-secondary);
    font-weight: 600;
  }

  .refresh:disabled {
    opacity: 0.5;
  }

  .status {
    margin: 0 4px;
    font-size: var(--size-caption);
    color: var(--text-2);
  }

  .stops.busy {
    opacity: 0.6;
  }

  .stops li + li {
    border-top: 1px solid var(--separator);
  }

  .stops a {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px 12px;
    padding: 12px 0;
  }

  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .code {
    font-size: var(--size-secondary);
    color: var(--text-2);
  }

  .distance {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .lines {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .empty,
  .help {
    padding: 16px var(--gutter);
    font-size: var(--size-secondary);
    color: var(--text-2);
  }

  .help {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .help p,
  .help ul {
    margin: 0;
  }

  .help ul {
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .help strong,
  .help b {
    color: var(--text);
  }

  a:not(.stops a) {
    color: var(--accent);
    font-weight: 600;
  }
</style>
