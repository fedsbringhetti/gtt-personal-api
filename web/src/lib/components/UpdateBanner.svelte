<script lang="ts">
  import { applyUpdate, update } from '../update.svelte'
  import Icon from './Icon.svelte'
</script>

{#if update.available}
  <div class="banner" role="status">
    <span class="icon"><Icon name="refresh" size={22} stroke={2.2} /></span>
    <span class="text">
      <strong>Nuova versione disponibile</strong>
      <span>Tocca per aggiornare l'app</span>
    </span>
    <button type="button" onclick={applyUpdate}>Aggiorna</button>
  </div>
{/if}

<style>
  /* Dark in both themes, so it stands out from the content; above the bottom bar. */
  .banner {
    position: fixed;
    left: 12px;
    right: 12px;
    bottom: calc(var(--tab-bar-height) + env(safe-area-inset-bottom) + 12px);
    z-index: 20;
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: 616px;
    margin: 0 auto;
    padding: 12px 12px 12px 16px;
    border-radius: var(--radius-card);
    background: #1c1c1e;
    color: #ffffff;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
  }

  .icon {
    color: #4fc3dc;
  }

  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .text strong {
    font-size: var(--size-secondary);
  }

  .text span {
    font-size: var(--size-caption);
    color: #aeaeb2;
  }

  button {
    height: 36px;
    padding: 0 14px;
    border: 0;
    border-radius: 18px;
    background: #4fc3dc;
    color: #000000;
    font-size: var(--size-secondary);
    font-weight: 700;
  }
</style>
