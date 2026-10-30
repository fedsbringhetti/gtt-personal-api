<script lang="ts">
  import { describeStatus, fetchStatus } from './lib/status'

  // Started once when the component is created; the `{#await}` block below follows it.
  const status = fetchStatus()
</script>

<main>
  <h1>GTT</h1>
  <p>La web app è in costruzione.</p>

  <p class="status">
    {#await status}
      Controllo il server…
    {:then value}
      {describeStatus(value)}
    {:catch error}
      Server non raggiungibile ({error.message})
    {/await}
  </p>
</main>

<style>
  /* Styles in a component apply only to its own elements. */
  .status {
    color: GrayText;
    font-size: 0.9rem;
  }
</style>
