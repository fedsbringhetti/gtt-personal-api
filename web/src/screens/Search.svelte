<script lang="ts">
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import Placeholder from './Placeholder.svelte'

  interface Props {
    query: string
  }

  let { query }: Props = $props()
</script>

<ScreenHeader title="Cerca" />
<Placeholder text={query ? `Risultati per «${query}» in arrivo.` : 'Qui arriva la ricerca delle fermate per nome, via o codice.'} />
