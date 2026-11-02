<script lang="ts">
  interface Props {
    /** What this screen will show, and in which step it arrives. */
    text: string
  }

  let { text }: Props = $props()
</script>

<p class="card">{text}</p>

<style>
  p {
    padding: 16px var(--gutter);
    color: var(--text-2);
    font-size: var(--size-secondary);
  }
</style>
