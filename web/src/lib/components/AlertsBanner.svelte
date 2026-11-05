<script lang="ts">
  import { alertsCount, alertsTitles, linkLabel } from '../alerts'
  import type { Alert } from '../api'
  import Icon from './Icon.svelte'
  import LineBadge from './LineBadge.svelte'

  interface Props {
    alerts: Alert[]
  }

  let { alerts }: Props = $props()

  // Closed by default (Phase 3, decision AA): alerts last for months, and their full text
  // every time would push the departures down.
  let open = $state(false)
</script>

<section class="banner" aria-label="Avvisi GTT">
  <button type="button" class="summary" aria-expanded={open} onclick={() => (open = !open)}>
    <Icon name="warning" size={22} />
    <span class="text">
      <span class="count">{alertsCount(alerts)}</span>
      {#if !open}<span class="titles">{alertsTitles(alerts)}</span>{/if}
    </span>
    <Icon name={open ? 'collapse' : 'expand'} size={20} stroke={2.4} />
  </button>
  {#if open}
    <div class="list">
      {#each alerts as alert (alert.id)}
        <article>
          <div class="heading">
            {#each alert.lines as line (line.id)}
              <LineBadge name={line.name} mode={line.mode} size="small" />
            {/each}
            <h2>{alert.title}</h2>
          </div>
          {#if alert.description}<p>{alert.description}</p>{/if}
          {#if alert.url}
            <a href={alert.url} target="_blank" rel="noopener noreferrer">
              {linkLabel(alert.url)}<Icon name="forward" size={16} stroke={2.4} />
            </a>
          {/if}
        </article>
      {/each}
    </div>
  {/if}
</section>

<style>
  .banner {
    display: flex;
    flex-direction: column;
    border-radius: var(--radius-card);
    background: var(--warning-bg);
    color: var(--warning);
  }

  .summary {
    display: grid;
    grid-template-columns: 22px minmax(0, 1fr) 20px;
    align-items: center;
    gap: 10px;
    min-height: 48px;
    padding: 10px 14px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
  }

  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .count {
    font-size: var(--size-secondary);
    font-weight: 700;
  }

  .titles {
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .list {
    display: flex;
    flex-direction: column;
    padding: 0 14px 6px;
  }

  article {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 0;
    border-top: 1px solid color-mix(in srgb, currentColor 20%, transparent);
  }

  .heading {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px 8px;
  }

  h2 {
    /* Beside the badges while there is room for a few words, otherwise on its own line. */
    flex: 1 1 10em;
    min-width: 0;
    margin: 0;
    font-size: var(--size-secondary);
    font-weight: 700;
  }

  p {
    margin: 0;
    font-size: 14px;
    line-height: 1.4;
    color: var(--text);
    /* GTT's texts come in lines: dates, then each direction. */
    white-space: pre-line;
    overflow-wrap: anywhere;
  }

  a {
    align-self: flex-start;
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 32px;
    font-size: 14px;
    font-weight: 600;
    color: inherit;
  }
</style>
