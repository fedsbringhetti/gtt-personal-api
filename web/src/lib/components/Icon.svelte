<script lang="ts" module>
  // Stroke icons on a 24 × 24 grid, drawn for this app (no icon library). Each entry is the
  // inside of the `<svg>`; `currentColor` makes an icon take the color of its text.
  const icons = {
    home: '<path d="M3 10.5L12 3l9 7.5"/><path d="M5 9.5V20h5v-6h4v6h5V9.5"/>',
    search: '<circle cx="11" cy="11" r="7"/><path d="M20 20l-3.5-3.5"/>',
    pin: '<path d="M12 21s-7-6.2-7-11.5A7 7 0 0 1 19 9.5C19 14.8 12 21 12 21z"/><circle cx="12" cy="9.5" r="2.5"/>',
    star: '<path d="M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.8l-5.2 2.8 1-5.8-4.3-4.1 5.9-.9z"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6"/><path d="M12 7.5v.01"/>',
    back: '<path d="M15 5l-7 7 7 7"/>',
    forward: '<path d="M9 5l7 7-7 7"/>',
    realtime: '<path d="M5 19a14 14 0 0 1 14-14"/><path d="M9 19a10 10 0 0 1 10-10"/><path d="M13 19a6 6 0 0 1 6-6"/>',
    up: '<path d="M12 19V5"/><path d="M6 11l6-6 6 6"/>',
    down: '<path d="M12 5v14"/><path d="M6 13l6 6 6-6"/>',
    trash: '<path d="M4 7h16"/><path d="M9 7V4h6v3"/><path d="M6 7l1 13h10l1-13"/>',
    share: '<path d="M12 3v12"/><path d="M8 7l4-4 4 4"/><path d="M5 12v8h14v-8"/>',
    refresh: '<path d="M20 11a8 8 0 1 0-2.3 5.7"/><path d="M20 5v6h-6"/>',
    offline:
      '<path d="M3 3l18 18"/><path d="M5 12.5a10 10 0 0 1 4-2.3"/><path d="M8.5 16a5 5 0 0 1 5.5-1"/><path d="M12 19.5v.01"/><path d="M13.5 6.1A15 15 0 0 1 22 9"/><path d="M2 9a15 15 0 0 1 3.4-2"/>',
    ok: '<circle cx="12" cy="12" r="9"/><path d="M8 12.5l2.7 2.7L16 9.8"/>',
  } as const

  export type IconName = keyof typeof icons
</script>

<script lang="ts">
  interface Props {
    name: IconName
    size?: number
    /** Line thickness on the 24 px grid. */
    stroke?: number
    /** Filled shapes, e.g. the star of a saved favorite. */
    filled?: boolean
    /** Read by screen readers; without it the icon is decoration and hidden from them. */
    label?: string
  }

  let { name, size = 24, stroke = 2, filled = false, label }: Props = $props()
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={filled ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
>
  <!-- The paths are constants above, never data from outside. -->
  {@html icons[name]}
</svg>

<style>
  svg {
    flex-shrink: 0;
    display: block;
  }
</style>
