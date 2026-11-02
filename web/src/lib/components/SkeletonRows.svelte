<script lang="ts">
  interface Props {
    count?: number
  }

  let { count = 4 }: Props = $props()

  // Different widths, so the placeholders look like text rather than a grid.
  const widths = [70, 55, 75, 60, 65]
</script>

<ul class="skeleton" aria-hidden="true">
  {#each { length: count }, index}
    <li class="row">
      <span class="badge"></span>
      <span class="lines">
        <span class="line" style:width="{widths[index % widths.length]}%"></span>
        <span class="line short" style:width="{widths[index % widths.length] - 30}%"></span>
      </span>
      <span class="minutes"></span>
    </li>
  {/each}
</ul>

<style>
  .skeleton {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .row {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr) 44px;
    align-items: center;
    gap: 10px;
    min-height: 64px;
    animation: pulse 1.4s ease-in-out infinite;
  }

  .row + .row {
    border-top: 1px solid var(--separator);
  }

  .badge,
  .line,
  .minutes {
    display: block;
    background: var(--skeleton);
  }

  .badge {
    width: 40px;
    height: 28px;
    border-radius: var(--radius-badge);
  }

  .lines {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .line {
    height: 12px;
    border-radius: 6px;
  }

  .short {
    height: 10px;
  }

  .minutes {
    width: 40px;
    height: 22px;
    border-radius: 6px;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 0.55;
    }
    50% {
      opacity: 1;
    }
  }
</style>
