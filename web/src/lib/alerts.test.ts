import { describe, expect, it } from 'vitest'
import type { Alert } from './api'
import { alertsCount, alertsForLines, alertsTitles, linesWithAlerts, linkLabel } from './alerts'

function alert(title: string, lines: string[]): Alert {
  return {
    id: title,
    title,
    description: null,
    url: null,
    lines: lines.map((name) => ({ id: `gtt:${name}U`, name, mode: 'BUS' })),
    start: null,
    end: null,
  }
}

const detour = alert('Linea 67 deviata', ['67'])
const suspended = alert('Fermata 750 sospesa', ['4', '67', '68+'])
const lifts = alert('Ascensori fuori servizio', [])

describe('the closed banner', () => {
  it('counts the alerts in Italian', () => {
    expect(alertsCount([detour])).toBe('1 avviso GTT')
    expect(alertsCount([detour, suspended])).toBe('2 avvisi GTT')
  })

  it('joins the titles', () => {
    expect(alertsTitles([detour, suspended])).toBe('Linea 67 deviata · Fermata 750 sospesa')
  })
})

describe('alertsForLines', () => {
  const all = [detour, suspended, lifts]

  it('keeps everything without a filter', () => {
    expect(alertsForLines(all, [])).toEqual(all)
  })

  it('keeps the alerts of the chosen lines and those naming no line', () => {
    expect(alertsForLines(all, ['4'])).toEqual([suspended, lifts])
    expect(alertsForLines(all, ['67'])).toEqual(all)
    expect(alertsForLines(all, ['68+'])).toEqual([suspended, lifts])
    expect(alertsForLines(all, ['9'])).toEqual([lifts])
  })

  it('ignores case, as line filters do', () => {
    expect(alertsForLines([alert('Metro', ['M1'])], ['m1'])).toHaveLength(1)
  })
})

describe('linesWithAlerts', () => {
  it('collects the names, lowercase', () => {
    expect(linesWithAlerts([detour, suspended, alert('Metro', ['M1'])])).toEqual(
      new Set(['67', '4', '68+', 'm1']),
    )
  })
})

describe('linkLabel', () => {
  it('names the site', () => {
    expect(linkLabel('http://www.gtt.to.it/cms/percorari/urbano')).toBe('Dettagli su gtt.to.it')
    expect(linkLabel('https://example.org/a')).toBe('Dettagli su example.org')
    expect(linkLabel('not a url')).toBe('Dettagli')
  })
})
