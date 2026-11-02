<script lang="ts">
  import Icon from '../lib/components/Icon.svelte'
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import Placeholder from './Placeholder.svelte'
</script>

<ScreenHeader title="Partenze">
  {#snippet actions()}
    <a class="icon-button" href="/info" aria-label="Informazioni">
      <Icon name="info" size={26} stroke={1.8} />
    </a>
  {/snippet}
</ScreenHeader>
<Placeholder text="Qui arrivano la ricerca, «vicino a me» e i preferiti con le prossime partenze." />

<style>
  .icon-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--touch);
    height: var(--touch);
    color: var(--accent);
  }
</style>
