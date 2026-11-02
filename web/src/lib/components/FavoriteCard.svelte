<script lang="ts">
  import {
    ApiError,
    DEPARTURES_MINUTES,
    errorText,
    stopParamFromId,
    type FavoriteDepartures,
  } from '../api'
  import { displayName, formatTime } from '../format'
  import { pathOf } from '../router'
  import DepartureRow from './DepartureRow.svelte'

  interface Props {
    favorite: FavoriteDepartures
  }

  let { favorite }: Props = $props()

  let stopName = $derived(favorite.stop ? displayName(favorite.stop.name) : null)
  let code = $derived(favorite.stop?.code ?? stopParamFromId(favorite.stop_id))
  // The chosen name, or the stop's own; the other side then shows the stop.
  let title = $derived(favorite.label ?? stopName ?? `Fermata ${code}`)
  // Two favorites of the same stop differ by their lines: "39 · solo 9".
  let only = $derived(favorite.lines.length ? ` · solo ${favorite.lines.join(', ')}` : '')
  let place = $derived((favorite.label && stopName ? `${stopName} · ${code}` : code) + only)
  let href = $derived(
    pathOf({ name: 'stop', stop: stopParamFromId(favorite.stop_id), lines: favorite.lines }),
  )
  let problem = $derived(
    favorite.error
      ? errorText(new ApiError(0, favorite.error.code, favorite.error.message))
      : null,
  )
</script>

<a class="card favorite" {href}>
  <span class="head">
    <span class="title">{title}</span>
    <span class="place">{place}</span>
  </span>
  {#if favorite.missing}
    <span class="note">Fermata non più servita: puoi toglierla dai preferiti.</span>
  {:else if problem}
    <span class="note">Partenze non disponibili. {problem}</span>
  {:else if favorite.departures.length === 0}
    <span class="note">
      Nessuna partenza {favorite.lines.length ? 'delle linee scelte ' : ''}nei prossimi
      {DEPARTURES_MINUTES} minuti.
    </span>
  {:else}
    <ol>
      {#each favorite.departures as departure (departure.trip_id + departure.scheduled)}
        <DepartureRow
          line={departure.line}
          mode={departure.mode}
          headsign={departure.headsign ? displayName(departure.headsign) : ''}
          time={formatTime(departure.expected)}
          minutes={departure.minutes}
          delay={departure.delay_seconds}
          realtime={departure.realtime}
          canceled={departure.canceled}
          compact
        />
      {/each}
    </ol>
    {#if favorite.stale}
      <span class="note">Il servizio degli orari non risponde: dati non aggiornati.</span>
    {/if}
  {/if}
</a>

<style>
  .favorite {
    display: flex;
    flex-direction: column;
    padding: 14px var(--gutter) 6px;
  }

  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 6px;
  }

  .title {
    flex-shrink: 0;
    max-width: 60%;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .place {
    min-width: 0;
    font-size: var(--size-secondary);
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  ol {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  ol :global(.row) {
    border-top: 1px solid var(--separator);
  }

  .note {
    padding: 10px 0 12px;
    border-top: 1px solid var(--separator);
    font-size: var(--size-secondary);
    color: var(--text-2);
  }
</style>
