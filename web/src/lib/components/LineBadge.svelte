<script lang="ts">
  import { vehicleType } from '../lines'

  interface Props {
    /** The line as people know it: `4`, `9B`, `M1`, `SFM1`. Always a string. */
    name: string
    /** The upstream mode (`BUS`, `TRAM`, ...), which picks the color. */
    mode: string | null | undefined
    size?: 'normal' | 'small'
  }

  let { name, mode, size = 'normal' }: Props = $props()
</script>

<span class="badge {vehicleType(mode)} {size}">{name}</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-sizing: border-box;
    min-width: 40px;
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius-badge);
    color: #ffffff;
    font-size: var(--size-secondary);
    font-weight: 700;
    white-space: nowrap;
  }

  .small {
    min-width: 32px;
    height: 24px;
    padding: 0 6px;
    border-radius: 6px;
    font-size: var(--size-caption);
  }

  .bus {
    background: var(--bus);
  }
  .tram {
    background: var(--tram);
  }
  .metro {
    background: var(--metro);
  }
  .rail {
    background: var(--rail);
  }
  .other {
    background: var(--other);
  }
</style>
