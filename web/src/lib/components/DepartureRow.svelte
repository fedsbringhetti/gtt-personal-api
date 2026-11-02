<script lang="ts">
  import { describeDelay } from '../lines'
  import Icon from './Icon.svelte'
  import LineBadge from './LineBadge.svelte'

  interface Props {
    line: string
    mode: string | null | undefined
    headsign: string
    /** Expected time, `HH:MM`. */
    time: string
    /** Minutes until departure; 0 shows "in arrivo". */
    minutes: number
    /** Delay in seconds, when known from real-time data. */
    delay: number | null
    realtime: boolean
    /** Old data (offline): the clock time replaces the countdown, which would mislead. */
    stale?: boolean
    /** The run will not pass: the row stays, so it is clear that this one is gone. */
    canceled?: boolean
    /** One line, smaller minutes: the rows of a favorite on the home screen. */
    compact?: boolean
  }

  let {
    line,
    mode,
    headsign,
    time,
    minutes,
    delay,
    realtime,
    stale = false,
    canceled = false,
    compact = false,
  }: Props = $props()

  let delayInfo = $derived(
    realtime && delay !== null && !canceled ? describeDelay(delay) : null,
  )
</script>

<li class="row" class:canceled class:compact>
  <LineBadge name={line} {mode} />
  <span class="middle">
    <span class="headsign">{headsign}</span>
    {#if compact}
      <!-- Compact rows have no second line. -->
    {:else if canceled}
      <span class="detail"><s>{time}</s></span>
    {:else if !stale}
      <span class="detail">
        {#if realtime}
          <span class="realtime"><Icon name="realtime" size={13} stroke={2.6} label="in tempo reale" /></span>
          {time}
          {#if delayInfo}· <span class="delay {delayInfo.level}">{delayInfo.label}</span>{/if}
        {:else}
          {time} · orario programmato
        {/if}
      </span>
    {/if}
  </span>
  {#if canceled}
    <span class="gone">soppressa</span>
  {:else if stale}
    <span class="clock">alle {time}</span>
  {:else if minutes <= 0}
    <span class="arriving">in arrivo</span>
  {:else}
    <span class="minutes">
      {#if compact && realtime}
        <span class="realtime"><Icon name="realtime" size={14} stroke={2.4} label="in tempo reale" /></span>
      {/if}
      <span class="number">{minutes}</span> min
    </span>
  {/if}
</li>

<style>
  .row {
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    min-height: 64px;
  }

  .row + :global(.row) {
    border-top: 1px solid var(--separator);
  }

  .row > :global(.badge) {
    justify-self: start;
  }

  .middle {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .headsign {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .detail {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 14px;
    color: var(--text-2);
  }

  .realtime {
    color: var(--accent);
  }

  .delay {
    font-weight: 600;
  }
  .on-time {
    color: var(--on-time);
  }
  .late {
    color: var(--late);
  }
  .very-late {
    color: var(--very-late);
  }
  .early {
    color: var(--early);
  }

  .minutes {
    font-size: 14px;
    color: var(--text-2);
    white-space: nowrap;
  }

  .number {
    font-size: 26px;
    font-weight: 700;
    color: var(--text);
    font-variant-numeric: tabular-nums;
  }

  .arriving {
    font-weight: 700;
    color: var(--accent);
  }

  .compact {
    grid-template-columns: 52px minmax(0, 1fr) auto;
    min-height: 44px;
  }

  .compact .headsign {
    font-size: 16px;
  }

  .compact .minutes {
    display: flex;
    align-items: baseline;
    gap: 4px;
  }

  .compact .number {
    font-size: 20px;
  }

  .canceled .headsign {
    color: var(--text-2);
  }

  .gone {
    font-weight: 600;
    color: var(--danger);
  }

  .clock {
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
</style>
