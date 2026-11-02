<script lang="ts">
  import { errorText, getFavoritesDepartures, type FavoriteDepartures } from '../lib/api'
  import ErrorCard from '../lib/components/ErrorCard.svelte'
  import FavoriteCard from '../lib/components/FavoriteCard.svelte'
  import Icon from '../lib/components/Icon.svelte'
  import Notice from '../lib/components/Notice.svelte'
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import SkeletonRows from '../lib/components/SkeletonRows.svelte'
  import { formatTime } from '../lib/format'
  import { startPolling } from '../lib/poll'

  /** How often the favorites' departures are reloaded while the screen is visible. */
  const REFRESH_MS = 30_000

  let data = $state<{ generated_at: string; favorites: FavoriteDepartures[] } | null>(null)
  let failure = $state<string | null>(null)
  let reload: () => void = () => {}

  $effect(() => {
    let call: AbortController | undefined
    const load = () => {
      call?.abort()
      const current = new AbortController()
      call = current
      getFavoritesDepartures(current.signal)
        .then((response) => {
          data = response
          failure = null
        })
        .catch((error: unknown) => {
          if (!current.signal.aborted) failure = errorText(error)
        })
    }
    reload = load
    const stopPolling = startPolling(load, REFRESH_MS)
    return () => {
      stopPolling()
      call?.abort()
    }
  })
</script>

<ScreenHeader title="Partenze">
  {#snippet actions()}
    <a class="icon-button" href="/info" aria-label="Informazioni">
      <Icon name="info" size={26} stroke={1.8} />
    </a>
  {/snippet}
</ScreenHeader>

<a class="search" href="/cerca">
  <Icon name="search" size={20} />
  Cerca fermata, via o codice
</a>

<a class="near" href="/vicino">
  <Icon name="pin" size={22} />
  <span>Fermate vicino a me</span>
  <Icon name="forward" size={18} stroke={2.2} />
</a>

<div class="section">
  <h2>Preferiti</h2>
  {#if data?.favorites.length}
    <a href="/preferiti">Modifica</a>
  {/if}
</div>

{#if data}
  {#if failure}
    <Notice message="Non riesco ad aggiornare: dati delle {formatTime(data.generated_at)}." retry={() => reload()} />
  {/if}
  {#if data.favorites.length === 0}
    <div class="card empty">
      <p>Nessun preferito per ora.</p>
      <p>Apri una fermata e tocca la stella: comparirà qui con le prossime partenze.</p>
      <a href="/cerca">Cerca una fermata</a>
    </div>
  {:else}
    {#each data.favorites as favorite (favorite.id)}
      <FavoriteCard {favorite} />
    {/each}
    <p class="updated">Aggiornato alle {formatTime(data.generated_at)} · si aggiorna da solo</p>
  {/if}
{:else if failure}
  <ErrorCard message={failure} retry={() => reload()} />
{:else}
  <div class="card"><SkeletonRows count={3} /></div>
{/if}

<style>
  .icon-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--touch);
    height: var(--touch);
    color: var(--accent);
  }

  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: var(--touch);
    padding: 0 12px;
    border-radius: var(--radius-field);
    background: var(--fill);
    color: var(--text-2);
  }

  .near {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 48px;
    padding: 0 14px;
    border-radius: var(--radius-field);
    background: var(--surface);
    color: var(--accent);
    font-weight: 600;
  }

  .near span {
    flex: 1;
  }

  .section {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-top: 6px;
  }

  h2 {
    margin: 0;
    font-size: var(--size-section);
    font-weight: 700;
  }

  .section a {
    color: var(--accent);
  }

  .empty {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px var(--gutter);
    font-size: var(--size-secondary);
    color: var(--text-2);
  }

  .empty p {
    margin: 0;
  }

  .empty p:first-child {
    color: var(--text);
    font-weight: 600;
  }

  .empty a {
    margin-top: 4px;
    color: var(--accent);
    font-weight: 600;
  }

  .updated {
    margin: 0;
    text-align: center;
    font-size: var(--size-caption);
    color: var(--text-2);
  }
</style>
