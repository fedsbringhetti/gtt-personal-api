<script lang="ts">
  import { navigation } from '../navigation.svelte'
  import { tabs } from '../tabs'
  import Icon from './Icon.svelte'
</script>

<nav aria-label="Navigazione principale">
  {#each tabs as entry (entry.tab)}
    {@const active = navigation.tab === entry.tab}
    <a href={entry.path} class:active aria-current={active ? 'page' : undefined}>
      <Icon name={entry.icon} size={26} filled={active && entry.icon === 'star'} />
      {entry.label}
    </a>
  {/each}
</nav>

<style>
  nav {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 10;
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    height: var(--tab-bar-height);
    padding: 0 8px env(safe-area-inset-bottom);
    background: var(--surface);
    border-top: 1px solid var(--separator);
  }

  a {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    color: var(--text-2);
    font-size: 11px;
    font-weight: 500;
  }

  .active {
    color: var(--accent);
    font-weight: 600;
  }
</style>
