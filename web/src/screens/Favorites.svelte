<script lang="ts">
  import { onDestroy } from 'svelte'
  import {
    errorText,
    getFavorites,
    removeFavorite,
    stopParamFromId,
    updateFavorite,
    type Favorite,
  } from '../lib/api'
  import ErrorCard from '../lib/components/ErrorCard.svelte'
  import Icon from '../lib/components/Icon.svelte'
  import Notice from '../lib/components/Notice.svelte'
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import SkeletonRows from '../lib/components/SkeletonRows.svelte'
  import { isChosen, toggleLine } from '../lib/departures'
  import { displayName } from '../lib/format'
  import { vehicleType } from '../lib/lines'

  /** How long "Conferma rimozione" waits for the second tap. */
  const CONFIRM_MS = 4000

  let favorites = $state<Favorite[] | null>(null)
  let loadFailure = $state<string | null>(null)
  let saveFailure = $state<string | null>(null)
  /** A change is being saved: buttons wait, so two taps cannot cross. */
  let busy = $state(false)
  /** The favorite whose "Rimuovi" was tapped once. */
  let confirming = $state<number | null>(null)
  let confirmTimer: ReturnType<typeof setTimeout> | undefined

  async function load() {
    loadFailure = null
    try {
      favorites = await getFavorites()
    } catch (error) {
      loadFailure = errorText(error)
    }
  }

  // Every change is saved at once, then the list is read again: positions and names always
  // match what the server has.
  async function save(change: () => Promise<unknown>) {
    busy = true
    saveFailure = null
    try {
      await change()
      favorites = await getFavorites()
    } catch (error) {
      saveFailure = errorText(error)
    } finally {
      busy = false
    }
  }

  function rename(favorite: Favorite, value: string) {
    const label = value.trim() || null
    if (label === favorite.label) return
    save(() => updateFavorite(favorite.id, { label }))
  }

  function move(favorite: Favorite, by: number) {
    save(() => updateFavorite(favorite.id, { position: favorite.position + by }))
  }

  function chooseLine(favorite: Favorite, line: string) {
    save(() => updateFavorite(favorite.id, { lines: toggleLine(favorite.lines, line) }))
  }

  function remove(favorite: Favorite) {
    clearTimeout(confirmTimer)
    if (confirming !== favorite.id) {
      confirming = favorite.id
      confirmTimer = setTimeout(() => (confirming = null), CONFIRM_MS)
      return
    }
    confirming = null
    save(() => removeFavorite(favorite.id))
  }

  load()
  onDestroy(() => clearTimeout(confirmTimer))
</script>

<ScreenHeader title="Preferiti" />

{#if saveFailure}
  <Notice message="Modifica non salvata. {saveFailure}" />
{/if}

{#if favorites === null && loadFailure}
  <ErrorCard message={loadFailure} retry={load} />
{:else if favorites === null}
  <div class="card"><SkeletonRows count={3} /></div>
{:else if favorites.length === 0}
  <div class="card empty">
    <p>Nessun preferito per ora.</p>
    <p>Apri una fermata e tocca la stella per aggiungerla.</p>
    <a href="/cerca">Cerca una fermata</a>
  </div>
{:else}
  <p class="intro">Nome, linee da mostrare e ordine in Home. Le modifiche si salvano subito.</p>
  {#each favorites as favorite, index (favorite.id)}
    {@const stopName = favorite.stop ? displayName(favorite.stop.name) : null}
    {@const code = favorite.stop?.code ?? stopParamFromId(favorite.stop_id)}
    <section class="card favorite" aria-label={favorite.label ?? stopName ?? `Fermata ${code}`}>
      <div class="top">
        <label class="name">
          <span>Nome</span>
          <input
            type="text"
            value={favorite.label ?? ''}
            placeholder={stopName ?? `Fermata ${code}`}
            maxlength="100"
            enterkeyhint="done"
            onchange={(event) => rename(favorite, event.currentTarget.value)}
            onkeydown={(event) => event.key === 'Enter' && event.currentTarget.blur()}
          />
        </label>
        <button
          type="button"
          class="order"
          aria-label="Sposta su"
          disabled={busy || index === 0}
          onclick={() => move(favorite, -1)}
        >
          <Icon name="up" size={20} stroke={2.2} />
        </button>
        <button
          type="button"
          class="order"
          aria-label="Sposta giù"
          disabled={busy || index === favorites.length - 1}
          onclick={() => move(favorite, 1)}
        >
          <Icon name="down" size={20} stroke={2.2} />
        </button>
      </div>

      <a class="stop" href="/fermata/{encodeURIComponent(stopParamFromId(favorite.stop_id))}">
        {stopName ?? 'Fermata'} · {code}
      </a>

      {#if favorite.missing}
        <p class="hint">Questa fermata non è più servita.</p>
      {:else if favorite.stop && favorite.stop.lines.length > 1}
        <div class="lines" role="group" aria-label="Linee da mostrare">
          {#each favorite.stop.lines as line (line.id)}
            <button
              type="button"
              class="line {vehicleType(line.mode)}"
              class:chosen={isChosen(favorite.lines, line.name)}
              aria-pressed={isChosen(favorite.lines, line.name)}
              disabled={busy}
              onclick={() => chooseLine(favorite, line.name)}
            >
              {line.name}
            </button>
          {/each}
        </div>
        <p class="hint">
          {favorite.lines.length
            ? `Solo ${favorite.lines.join(', ')}. Tocca una linea per aggiungerla o toglierla.`
            : 'Tutte le linee. Tocca una linea per mostrare solo quella.'}
        </p>
      {/if}

      <button
        type="button"
        class="remove"
        class:confirming={confirming === favorite.id}
        disabled={busy}
        onclick={() => remove(favorite)}
      >
        <Icon name="trash" size={18} />
        {confirming === favorite.id ? 'Conferma rimozione' : 'Rimuovi'}
      </button>
    </section>
  {/each}
{/if}

<style>
  .intro {
    margin: 0 4px;
    font-size: 14px;
    color: var(--text-2);
  }

  .favorite {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px var(--gutter) 6px;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .name {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .name span {
    font-size: 12px;
    color: var(--text-2);
  }

  .name input {
    min-width: 0;
    padding: 4px 0;
    border: 0;
    border-bottom: 1px solid var(--separator);
    background: transparent;
    font-weight: 600;
    outline: none;
  }

  .name input:focus {
    border-bottom-color: var(--accent);
  }

  .order {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--touch);
    height: var(--touch);
    padding: 0;
    border: 0;
    border-radius: var(--radius-field);
    background: var(--fill);
  }

  .order:disabled {
    opacity: 0.4;
  }

  .stop {
    align-self: flex-start;
    font-size: var(--size-secondary);
    color: var(--text-2);
  }

  .lines {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .line {
    height: 32px;
    min-width: 40px;
    padding: 0 10px;
    border-radius: 8px;
    border: 1px dashed var(--text-2);
    background: transparent;
    color: var(--text-2);
    font-size: 14px;
    font-weight: 700;
  }

  .line.chosen {
    border: 0;
    color: #ffffff;
  }
  .line.chosen.bus {
    background: var(--bus);
  }
  .line.chosen.tram {
    background: var(--tram);
  }
  .line.chosen.metro {
    background: var(--metro);
  }
  .line.chosen.rail {
    background: var(--rail);
  }
  .line.chosen.other {
    background: var(--other);
  }

  .hint {
    margin: 0;
    font-size: var(--size-caption);
    color: var(--text-2);
  }

  .remove {
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: var(--touch);
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--danger);
  }

  .remove.confirming {
    font-weight: 700;
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
</style>
