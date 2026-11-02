<script lang="ts">
  import { highlight } from '../format'

  interface Props {
    text: string
    /** The words to show in bold. */
    query: string
  }

  let { text, query }: Props = $props()
</script>

{#each highlight(text, query) as part, index (index)}{#if part.match}<b>{part.text}</b>{:else}{part.text}{/if}{/each}

<style>
  b {
    font-weight: 700;
    color: var(--text);
  }
</style>
