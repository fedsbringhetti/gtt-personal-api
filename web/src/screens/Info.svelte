<script lang="ts">
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import { describeStatus, fetchStatus } from '../lib/status'

  // Started once when the screen is created; the `{#await}` block below follows it.
  const status = fetchStatus()
</script>

<ScreenHeader title="Info" back />
<p class="card">
  {#await status}
    Controllo il server…
  {:then value}
    {describeStatus(value)}
  {:catch error}
    Server non raggiungibile ({error.message})
  {/await}
</p>

<style>
  p {
    padding: 16px var(--gutter);
    font-size: var(--size-secondary);
    color: var(--text-2);
  }
</style>
