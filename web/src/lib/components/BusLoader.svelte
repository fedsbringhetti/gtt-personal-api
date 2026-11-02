<script lang="ts">
  interface Props {
    /** What is loading, read by screen readers too. */
    label?: string
  }

  let { label = 'Carico…' }: Props = $props()
</script>

<div class="loader" role="status">
  <div class="track" aria-hidden="true">
    <svg class="bus" width="36" height="24" viewBox="0 0 36 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round">
      <rect x="2" y="3" width="30" height="15" rx="4" />
      <path d="M6 8h6v5H6zM15 8h6v5h-6zM24 8h5v5h-5z" />
      <circle cx="9" cy="20" r="2.4" fill="currentColor" />
      <circle cx="26" cy="20" r="2.4" fill="currentColor" />
    </svg>
  </div>
  <span class="label">{label}</span>
</div>

<style>
  .loader {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 22px var(--gutter) 18px;
  }

  .track {
    position: relative;
    width: 250px;
    max-width: 100%;
    height: 30px;
    overflow: hidden;
  }

  /* The road: a dotted line under the wheels. */
  .track::before {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    top: 22px;
    border-top: 3px dotted var(--text-2);
  }

  .bus {
    position: absolute;
    top: 0;
    left: 0;
    color: var(--accent);
    animation: drive 2.4s linear infinite;
  }

  @keyframes drive {
    from {
      transform: translateX(-36px);
    }
    to {
      transform: translateX(250px);
    }
  }

  .label {
    font-size: var(--size-secondary);
    color: var(--text-2);
  }
</style>
