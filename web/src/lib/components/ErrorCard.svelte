<script lang="ts">
  import Icon from './Icon.svelte'

  interface Props {
    message: string
    /** Shows a "Riprova" button that calls it. */
    retry?: () => void
  }

  let { message, retry }: Props = $props()
</script>

<div class="card error" role="alert">
  <p>{message}</p>
  {#if retry}
    <button type="button" onclick={retry}><Icon name="refresh" size={16} stroke={2.4} />Riprova</button>
  {/if}
</div>

<style>
  .error {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 16px var(--gutter);
  }

  p {
    margin: 0;
    font-size: var(--size-secondary);
  }

  button {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: var(--touch);
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--accent);
    font-weight: 600;
  }
</style>
