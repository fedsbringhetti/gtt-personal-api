import { describe, expect, it } from 'vitest'
import { displayName, formatDistance, formatTime, highlight } from './format'

describe('formatTime', () => {
  it('shows Turin time', () => {
    expect(formatTime('2026-10-29T17:24:00+01:00')).toBe('17:24')
    expect(formatTime('2026-10-29T16:24:00Z')).toBe('17:24')
  })

  it('pads hours and handles summer time', () => {
    expect(formatTime('2026-07-01T05:07:00Z')).toBe('07:07')
    expect(formatTime('2026-10-30T00:10:00+01:00')).toBe('00:10')
  })
})

describe('displayName', () => {
  it('turns capitals into a title', () => {
    expect(displayName('PORTA NUOVA')).toBe('Porta Nuova')
    expect(displayName('STRADA DEL DROSSO')).toBe('Strada del Drosso')
    expect(displayName('MONCALIERI, TESTONA - PIAZZA MARCONI')).toBe(
      'Moncalieri, Testona - Piazza Marconi',
    )
  })

  it('keeps codes, acronyms and Roman numerals', () => {
    expect(displayName('PORTA NUOVA - M1 - STAZIONE')).toBe('Porta Nuova - M1 - Stazione')
    expect(displayName('VITTORIO EMANUELE II CAP')).toBe('Vittorio Emanuele II Cap')
    expect(displayName('C. ROMA N.45 MONCALIERI')).toBe('C. Roma N.45 Moncalieri')
    expect(displayName('STAZIONE SFM DORA')).toBe('Stazione SFM Dora')
  })

  it('keeps abbreviations with a dot readable', () => {
    expect(displayName('ASTI (C.SO CASALE /V. TOSI)')).toBe('Asti (C.so Casale /V. Tosi)')
    expect(displayName('CALUSO (P.ZA MAZZINI)')).toBe('Caluso (P.za Mazzini)')
    expect(displayName('FAIALLO (FRAZ.CAMPETTI)')).toBe('Faiallo (Fraz.Campetti)')
    expect(displayName('PLAVA C.30')).toBe('Plava C.30')
  })

  it('handles apostrophes', () => {
    expect(displayName("SANT'ANNA")).toBe("Sant'Anna")
    expect(displayName("PIAZZA DELL'ARSENALE")).toBe("Piazza dell'Arsenale")
    expect(displayName("L'AQUILA")).toBe("L'Aquila")
  })

  it('capitalizes small words that start a name or a part', () => {
    expect(displayName('LA LOGGIA')).toBe('La Loggia')
    expect(displayName('ARSENALE (DI FRONTE)')).toBe('Arsenale (Di Fronte)')
  })

  it('leaves names that are already mixed case', () => {
    expect(displayName('Torino Porta Nuova')).toBe('Torino Porta Nuova')
  })
})

describe('highlight', () => {
  const marked = (text: string, query: string) =>
    highlight(text, query)
      .map((part) => (part.match ? `[${part.text}]` : part.text))
      .join('')

  it('marks every word of the query, ignoring case', () => {
    expect(marked('Porta Nuova', 'porta nuova')).toBe('[Porta] [Nuova]')
    expect(marked('Porta Nuova', 'nuova porta')).toBe('[Porta] [Nuova]')
  })

  it('matches short words only at the start of a word', () => {
    expect(marked('Porta Nuova', 'porta n')).toBe('[Porta] [N]uova')
    expect(marked('Vanchiglia', 'an')).toBe('Vanchiglia')
    expect(marked('Via Nizza', 'vi')).toBe('[Vi]a Nizza')
  })

  it('ignores accents', () => {
    expect(marked('Università', 'universita')).toBe('[Università]')
  })

  it('finds a match inside a word', () => {
    expect(marked('Arsenale (stazione Porta Nuova)', 'nuov')).toBe('Arsenale (stazione Porta [Nuov]a)')
  })

  it('returns the text as it is without a match', () => {
    expect(highlight('Bertola', 'xyz')).toEqual([{ text: 'Bertola', match: false }])
    expect(highlight('Bertola', '')).toEqual([{ text: 'Bertola', match: false }])
  })
})

describe('formatDistance', () => {
  it('rounds to 10 meters', () => {
    expect(formatDistance(0)).toBe('0 m')
    expect(formatDistance(83)).toBe('80 m')
    expect(formatDistance(85)).toBe('90 m')
    expect(formatDistance(994)).toBe('990 m')
  })

  it('switches to kilometers with an Italian comma', () => {
    expect(formatDistance(996)).toBe('1 km')
    expect(formatDistance(1240)).toBe('1,2 km')
    expect(formatDistance(1500)).toBe('1,5 km')
  })
})
