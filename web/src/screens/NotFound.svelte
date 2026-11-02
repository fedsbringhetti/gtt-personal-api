<script lang="ts">
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
</script>

<ScreenHeader title="Pagina non trovata" />
<p class="card">Questo indirizzo non corrisponde a nessuna schermata. <a href="/">Torna alla Home</a></p>

<style>
  p {
    padding: 16px var(--gutter);
    font-size: var(--size-secondary);
    color: var(--text-2);
  }

  a {
    color: var(--accent);
  }
</style>
