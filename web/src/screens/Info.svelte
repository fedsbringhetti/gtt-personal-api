<script lang="ts">
  import { onMount } from 'svelte'
  import { errorText, getStatus, type Status } from '../lib/api'
  import ErrorCard from '../lib/components/ErrorCard.svelte'
  import Icon from '../lib/components/Icon.svelte'
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import SkeletonRows from '../lib/components/SkeletonRows.svelte'
  import { describeStopIndex, describeUpstream } from '../lib/status'

  let status = $state<Status | null>(null)
  let failure = $state<string | null>(null)

  async function load() {
    failure = null
    try {
      status = await getStatus()
    } catch (error) {
      failure = errorText(error)
    }
  }

  // Opened from the Home Screen icon the app has its own window: instructions to install it
  // would be pointless. `navigator.standalone` is the iPhone's older way of saying so.
  const installed =
    matchMedia('(display-mode: standalone)').matches ||
    (navigator as Navigator & { standalone?: boolean }).standalone === true

  onMount(load)

  let upstream = $derived(status ? describeUpstream(status) : null)
  let stopIndex = $derived(status ? describeStopIndex(status) : null)
</script>

<ScreenHeader title="Info" back />

{#if status && upstream && stopIndex}
  <dl class="card">
    <div class="row">
      <dt>Versione</dt>
      <dd><span class="value">{status.version}</span></dd>
    </div>
    <div class="row">
      <dt>Orari in tempo reale</dt>
      <dd>
        <span class="value {upstream.tone}">
          {#if upstream.tone === 'ok'}<Icon name="ok" size={18} stroke={2.2} />{/if}
          {upstream.value}
        </span>
        {#if upstream.detail}<span class="detail">{upstream.detail}</span>{/if}
      </dd>
    </div>
    <div class="row">
      <dt>Elenco fermate</dt>
      <dd>
        <span class="value {stopIndex.tone}">{stopIndex.value}</span>
        {#if stopIndex.detail}<span class="detail">{stopIndex.detail}</span>{/if}
      </dd>
    </div>
  </dl>
  {#if status.upstream.ok === false && status.upstream.last_error}
    <p class="technical">Dettaglio tecnico: {status.upstream.last_error}</p>
  {/if}
{:else if failure}
  <ErrorCard message={failure} retry={load} />
{:else}
  <div class="card"><SkeletonRows count={3} /></div>
{/if}

<h2>{installed ? 'App' : 'Installa sul telefono'}</h2>
{#if installed}
  <p class="card installed">
    <Icon name="ok" size={20} stroke={2.2} />
    InTime è installata su questo telefono.
  </p>
{:else}
  <div class="card install">
    <div>
      <h3>iPhone, con Safari</h3>
      <ol>
        <li>
          Tocca <span class="share"><Icon name="share" size={18} label="Condividi" /></span> Condividi
        </li>
        <li>Scegli «Aggiungi alla schermata Home»</li>
        <li>Apri InTime dall'icona: serve per le notifiche</li>
      </ol>
    </div>
    <div>
      <h3>Android, con Chrome</h3>
      <ol>
        <li>Tocca il menu ⋮ in alto a destra</li>
        <li>Scegli «Installa app»</li>
      </ol>
    </div>
  </div>
{/if}

<p class="footer">Dati di GTT tramite muoversiatorino.it. Servizio personale, non ufficiale.</p>

<style>
  dl {
    display: flex;
    flex-direction: column;
  }

  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    min-height: 56px;
  }

  .row + .row {
    border-top: 1px solid var(--separator);
  }

  dd {
    margin: 0;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    text-align: right;
  }

  .value {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }

  .value.ok {
    color: var(--on-time);
    font-weight: 600;
  }

  .value.problem {
    color: var(--danger);
    font-weight: 600;
  }

  .detail {
    font-size: var(--size-caption);
    color: var(--text-2);
  }

  .technical {
    margin: 0 4px;
    font-size: var(--size-caption);
    color: var(--text-2);
    overflow-wrap: anywhere;
  }

  h2 {
    margin: 8px 4px 0;
    font-size: var(--size-caption);
    font-weight: 600;
    color: var(--text-2);
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }

  .install {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px var(--gutter);
  }

  .install > div + div {
    border-top: 1px solid var(--separator);
    padding-top: 12px;
  }

  h3 {
    margin: 0 0 6px;
    font-size: var(--size-body);
    font-weight: 600;
  }

  ol {
    margin: 0;
    padding-left: 20px;
    font-size: var(--size-secondary);
    line-height: 1.5;
  }

  .share {
    display: inline-flex;
    vertical-align: -3px;
    color: var(--accent);
  }

  .installed {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px var(--gutter);
    color: var(--on-time);
    font-weight: 600;
  }

  .footer {
    margin: 4px 4px 0;
    font-size: var(--size-caption);
    color: var(--text-2);
  }
</style>
