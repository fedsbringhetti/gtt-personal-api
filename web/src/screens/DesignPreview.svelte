<script lang="ts">
  // The design base on one page, to compare with the approved mockups (docs/plan/phase-2.md,
  // section 6). Reachable at /prova-grafica in development builds only (see App.svelte).
  import BusLoader from '../lib/components/BusLoader.svelte'
  import DepartureRow from '../lib/components/DepartureRow.svelte'
  import Icon, { type IconName } from '../lib/components/Icon.svelte'
  import ErrorCard from '../lib/components/ErrorCard.svelte'
  import LineBadge from '../lib/components/LineBadge.svelte'
  import Notice from '../lib/components/Notice.svelte'
  import ScreenHeader from '../lib/components/ScreenHeader.svelte'
  import SkeletonRows from '../lib/components/SkeletonRows.svelte'

  const colors = [
    ['Sfondo', '--bg'],
    ['Superficie', '--surface'],
    ['Testo', '--text'],
    ['Testo secondario', '--text-2'],
    ['Accento', '--accent'],
    ['In orario', '--on-time'],
    ['Ritardo', '--late'],
    ['Ritardo forte', '--very-late'],
  ]

  const badges = [
    { name: '67', mode: 'BUS', label: 'Bus' },
    { name: '9', mode: 'TRAM', label: 'Tram' },
    { name: 'M1', mode: 'SUBWAY', label: 'Metro' },
    { name: 'SFM1', mode: 'RAIL', label: 'Treno' },
    { name: 'Sassi-Superga', mode: 'FUNICULAR', label: 'Altro' },
  ]

  const icons: IconName[] = [
    'home',
    'search',
    'pin',
    'star',
    'info',
    'back',
    'forward',
    'realtime',
    'up',
    'down',
    'trash',
    'share',
    'refresh',
    'offline',
    'ok',
  ]
</script>

<ScreenHeader title="Prova grafica" subtitle="Solo in sviluppo: la base grafica della Fase 2" />

<h2>Colori</h2>
<ul class="card swatches">
  {#each colors as [label, variable] (variable)}
    <li>
      <span class="swatch" style:background="var({variable})"></span>
      {label}
      <code>{variable}</code>
    </li>
  {/each}
</ul>

<h2>Badge delle linee</h2>
<div class="card badges">
  {#each badges as badge (badge.mode)}
    <span class="badge-sample">
      <LineBadge name={badge.name} mode={badge.mode} />
      <LineBadge name={badge.name} mode={badge.mode} size="small" />
      <small>{badge.label}</small>
    </span>
  {/each}
</div>

<h2>Partenze</h2>
<ol class="card">
  <DepartureRow line="9" mode="TRAM" headsign="Stampalia" time="08:01" minutes={0} delay={60} realtime />
  <DepartureRow line="67" mode="BUS" headsign="Moncalieri, Testona" time="08:07" minutes={6} delay={120} realtime />
  <DepartureRow line="9B" mode="BUS" headsign="Corso Grosseto" time="08:12" minutes={11} delay={null} realtime={false} />
  <DepartureRow line="6" mode="BUS" headsign="Sassi" time="08:14" minutes={13} delay={10} realtime />
  <DepartureRow line="68" mode="BUS" headsign="Mongreno" time="08:16" minutes={15} delay={-60} realtime />
  <DepartureRow line="9" mode="TRAM" headsign="Stampalia" time="08:20" minutes={19} delay={360} realtime />
  <DepartureRow line="67" mode="BUS" headsign="Moncalieri, Testona" time="08:24" minutes={23} delay={0} realtime canceled />
</ol>

<h2>Avvisi ed errori</h2>
<Notice message="Il servizio degli orari non risponde: dati delle 08:01." />
<Notice message="Non riesco ad aggiornare: dati delle 08:01." retry={() => {}} />
<ErrorCard message="Server non raggiungibile: controlla la connessione." retry={() => {}} />

<h2>Offline</h2>
<div class="offline" role="status">
  <Icon name="offline" size={22} />
  <span><strong>Sei offline</strong><br />Dati delle 07:52: gli orari potrebbero essere cambiati</span>
</div>
<ol class="card">
  <DepartureRow line="M1" mode="SUBWAY" headsign="Bengasi" time="08:02" minutes={1} delay={0} realtime stale />
</ol>

<h2>Caricamento</h2>
<div class="card">
  <BusLoader label="Carico le partenze…" />
  <SkeletonRows count={3} />
</div>

<h2>Testo</h2>
<div class="card type">
  <span style:font-size="var(--size-title)" style:font-weight="700">Titolo 34</span>
  <span style:font-size="var(--size-section)" style:font-weight="700">Sezione 22</span>
  <span>Corpo 17 · nomi di fermate e destinazioni</span>
  <span style:font-size="var(--size-secondary)" style:color="var(--text-2)">Secondario 15 · codici</span>
  <span style:font-size="var(--size-caption)" style:color="var(--text-2)">Didascalia 13 · note</span>
</div>

<h2>Icone</h2>
<ul class="card icons">
  {#each icons as name (name)}
    <li><Icon {name} /><small>{name}</small></li>
  {/each}
</ul>

<style>
  h2 {
    margin: 10px 4px 0;
    font-size: var(--size-caption);
    font-weight: 600;
    color: var(--text-2);
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }

  .swatches li {
    display: grid;
    grid-template-columns: 28px 1fr auto;
    align-items: center;
    gap: 12px;
    min-height: 44px;
  }

  .swatches li + li {
    border-top: 1px solid var(--separator);
  }

  .swatch {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    box-shadow: inset 0 0 0 1px var(--separator);
  }

  code {
    font-size: var(--size-caption);
    color: var(--text-2);
  }

  .badges {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    padding: 16px var(--gutter);
  }

  .badge-sample {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }

  small {
    font-size: var(--size-caption);
    color: var(--text-2);
  }

  .offline {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    border-radius: var(--radius-field);
    background: var(--warning-bg);
    color: var(--warning);
    font-size: var(--size-caption);
  }

  .offline strong {
    font-size: var(--size-secondary);
  }

  .type {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px var(--gutter);
  }

  .icons {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(72px, 1fr));
    gap: 12px;
    padding: 16px var(--gutter);
  }

  .icons li {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
</style>
