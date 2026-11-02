<script lang="ts">
  import type { Snippet } from 'svelte'
  import { goBack, navigation } from '../navigation.svelte'
  import { tabInfo } from '../tabs'
  import Icon from './Icon.svelte'

  interface Props {
    title: string
    subtitle?: string
    /** A back link above the title, for screens opened from another one (stop, info). */
    back?: boolean
    /** Buttons on the right: the star on a stop, the info icon on Home. */
    actions?: Snippet
  }

  let { title, subtitle, back = false, actions }: Props = $props()

  // Back leads to the screen of the highlighted bottom bar entry.
  let origin = $derived(tabInfo(navigation.tab))

  function onBack(event: MouseEvent) {
    event.preventDefault()
    goBack(origin.path)
  }
</script>

<header>
  {#if back || actions}
    <div class="bar">
      {#if back}
        <a class="back" href={origin.path} onclick={onBack}>
          <Icon name="back" size={22} stroke={2.4} />{origin.label}
        </a>
      {/if}
      <span class="actions">{@render actions?.()}</span>
    </div>
  {/if}
  <h1 class:small={back}>{title}</h1>
  {#if subtitle}<p>{subtitle}</p>{/if}
</header>

<style>
  header {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: var(--touch);
    margin: 0 -8px;
  }

  .back {
    display: flex;
    align-items: center;
    gap: 2px;
    height: var(--touch);
    padding: 0 8px;
    color: var(--accent);
  }

  .actions {
    display: flex;
    margin-left: auto;
  }

  h1 {
    margin: 0;
    font-size: var(--size-title);
    font-weight: 700;
    letter-spacing: -0.5px;
  }

  /* A screen opened from another has a smaller title, as on the iPhone. */
  h1.small {
    font-size: 30px;
    letter-spacing: -0.4px;
  }

  p {
    margin: 0;
    font-size: var(--size-secondary);
    color: var(--text-2);
  }
</style>
