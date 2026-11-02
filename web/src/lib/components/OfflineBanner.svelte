<script lang="ts">
  import { formatTime } from '../format'
  import Icon from './Icon.svelte'

  interface Props {
    /** When the data on screen was received, ISO 8601. */
    since: string
  }

  let { since }: Props = $props()

  // The phone knows when it has no network; otherwise the network works and the server is down.
  let title = $derived(navigator.onLine ? 'Server non raggiungibile' : 'Sei offline')
</script>

<div class="offline" role="status">
  <Icon name="offline" size={22} />
  <span>
    <strong>{title}</strong>
    Dati delle {formatTime(since)}: gli orari potrebbero essere cambiati
  </span>
</div>

<style>
  .offline {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radius-field);
    background: var(--warning-bg);
    color: var(--warning);
    font-size: var(--size-caption);
  }

  span {
    display: flex;
    flex-direction: column;
  }

  strong {
    font-size: var(--size-secondary);
  }
</style>
