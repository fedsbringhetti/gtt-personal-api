import { describe, expect, it } from 'vitest'
import { filterDepartures, isChosen, lineChoices, toggleLine } from './departures'

const departures = [
  { line: '9', mode: 'TRAM' },
  { line: '9B', mode: 'BUS' },
  { line: '67', mode: 'BUS' },
  { line: 'M1S', mode: 'BUS' },
]

describe('lineChoices', () => {
  it('lists the stop lines first, then lines seen only in departures', () => {
    const stopLines = [
      { name: '9', mode: 'TRAM' },
      { name: '9B', mode: 'BUS' },
      { name: '67', mode: 'BUS' },
      { name: '68', mode: 'BUS' },
    ]
    expect(lineChoices(stopLines, departures).map((line) => line.name)).toEqual([
      '9',
      '9B',
      '67',
      '68',
      'M1S',
    ])
  })

  it('does not repeat a line written in another case', () => {
    expect(lineChoices([{ name: '9b', mode: 'BUS' }], [{ line: '9B', mode: 'BUS' }])).toHaveLength(1)
  })
})

describe('filterDepartures', () => {
  it('keeps everything without a choice', () => {
    expect(filterDepartures(departures, [])).toHaveLength(4)
  })

  it('keeps the chosen lines, ignoring case', () => {
    expect(filterDepartures(departures, ['9b', '67']).map((d) => d.line)).toEqual(['9B', '67'])
  })
})

describe('toggleLine', () => {
  it('adds and removes', () => {
    expect(toggleLine([], '4')).toEqual(['4'])
    expect(toggleLine(['4', '15'], '4')).toEqual(['15'])
    expect(toggleLine(['9B'], '9b')).toEqual([])
    expect(isChosen(['9B'], '9b')).toBe(true)
  })
})
