<script lang="ts">
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import Placeholder from './Placeholder.svelte'

  interface Props {
    /** The stop as written in the address: `39` or `gtt:39`. */
    stop: string
  }

  let { stop }: Props = $props()
</script>

<ScreenHeader title="Fermata {stop}" back />
<Placeholder text="Qui arrivano le partenze in tempo reale, con i filtri per linea." />
