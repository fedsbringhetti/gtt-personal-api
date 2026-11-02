<script lang="ts">
  import Icon from './Icon.svelte'

  interface Props {
    message: string
    /** Shows a "Riprova" button that calls it. */
    retry?: () => void
  }

  let { message, retry }: Props = $props()
</script>

<!-- A warning above data that is still shown: old data, a failed refresh. -->
<div class="notice" role="status">
  <span>{message}</span>
  {#if retry}
    <button type="button" onclick={retry}><Icon name="refresh" size={16} stroke={2.4} />Riprova</button>
  {/if}
</div>

<style>
  .notice {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 10px 14px;
    border-radius: var(--radius-field);
    background: var(--warning-bg);
    color: var(--warning);
    font-size: var(--size-caption);
  }

  button {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    min-height: 36px;
    padding: 0;
    border: 0;
    background: transparent;
    color: inherit;
    font-weight: 600;
  }
</style>
