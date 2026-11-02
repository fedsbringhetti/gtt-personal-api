/** Turning API values into the text the screens show. */

const timeFormat = new Intl.DateTimeFormat('it-IT', {
  hour: '2-digit',
  minute: '2-digit',
  hourCycle: 'h23',
  timeZone: 'Europe/Rome',
})

/** `2026-10-29T17:24:00+01:00` → `17:24`, in Turin's time whatever the phone's zone. */
export function formatTime(iso: string): string {
  return timeFormat.format(new Date(iso))
}

// Words written in lowercase inside a name, as in "Strada del Drosso".
const SMALL_WORDS = new Set([
  'a', 'ad', 'al', 'alla', 'alle', 'agli', 'ai', 'con', 'da', 'dal', 'dalla', 'dalle', 'dai',
  'degli', 'dei', 'del', 'della', 'delle', 'dello', 'di', 'e', 'ed', 'fra', 'gli', 'i', 'il',
  'in', 'la', 'le', 'lo', 'nel', 'nella', 'per', 'sul', 'sulla', 'su', 'tra',
])
// The same before an apostrophe: "dell'Arsenale", "all'Angelo".
const SMALL_ELISIONS = new Set(['all', 'dall', 'dell', 'nell', 'sull', 'l', 'd', 'un'])
const ACRONYMS = new Set(['GTT', 'SFM', 'FS', 'RFI', 'ASL', 'ATC', 'ITIS'])
const ROMAN = /^(?=[IVXLC]{2,}$)(C[DM]|D?C{0,3})(X[CL]|L?X{0,3})(I[XV]|V?I{0,3})$/

function capitalize(word: string): string {
  return word.charAt(0).toUpperCase() + word.slice(1).toLowerCase()
}

function titleWord(word: string, first: boolean): string {
  if (/\d/.test(word) || ACRONYMS.has(word) || ROMAN.test(word)) return word
  const lower = word.toLowerCase()
  if (!first && SMALL_WORDS.has(lower)) return lower
  return capitalize(word)
}

/**
 * Upstream names are in capitals ("PORTA NUOVA", "STRADA DEL DROSSO"); the app shows them
 * as titles ("Porta Nuova", "Strada del Drosso"). Codes and line names (`M1`, `9B`),
 * acronyms and Roman numerals stay as they are. A name that already has lowercase letters
 * (other companies) is left alone.
 */
export function displayName(name: string): string {
  if (/\p{Ll}/u.test(name)) return name
  // Words and the separators between them, kept so the result has the same punctuation.
  return name.replace(/[\p{L}\p{N}]+(?:'[\p{L}\p{N}]+)*/gu, (token, offset: number) => {
    const before = name.slice(0, offset)
    // A word is "first" at the start and after a dash, an opening bracket or a slash,
    // which start a new part of the name ("Porta Nuova - Stazione").
    const first = /(^|[-(/]\s*)$/.test(before)
    // The end of an abbreviation glued to a dot ("C.SO", "P.ZA", "V.LE") stays lowercase;
    // a longer word after a dot is a word of its own ("FRAZ.CAMPETTI").
    if (before.endsWith('.') && /^\p{L}{1,3}$/u.test(token) && /\p{L}\.$/u.test(before)) {
      return token.toLowerCase()
    }
    const parts = token.split("'")
    return parts
      .map((part, index) => {
        const isFirst = first && index === 0
        if (index < parts.length - 1 && !isFirst && SMALL_ELISIONS.has(part.toLowerCase())) {
          return part.toLowerCase()
        }
        return titleWord(part, isFirst)
      })
      .join("'")
  })
}

/** Folds case and accents, one character for one, so positions stay the same. */
function fold(text: string): string {
  return Array.from(text, (char) => char.normalize('NFD').charAt(0).toLowerCase()).join('')
}

export interface TextPart {
  text: string
  match: boolean
}

/**
 * Splits `text` into the parts that match the words of `query` (case and accents ignored)
 * and the rest, for showing matches in bold. As in the server's search, a word of 3 letters
 * or more matches anywhere, a shorter one only at the start of a word ("porta n").
 */
export function highlight(text: string, query: string): TextPart[] {
  const haystack = fold(text)
  const words = fold(query)
    .split(/[^\p{L}\p{N}]+/u)
    .filter((word) => word !== '')
  const marked: boolean[] = Array.from(text, () => false)
  const startsWord = (at: number) => at === 0 || !/[\p{L}\p{N}]/u.test(haystack[at - 1])
  for (const word of words) {
    for (let at = haystack.indexOf(word); at !== -1; at = haystack.indexOf(word, at + 1)) {
      if (word.length >= 3 || startsWord(at)) marked.fill(true, at, at + word.length)
    }
  }
  const chars = Array.from(text)
  const parts: TextPart[] = []
  chars.forEach((char, index) => {
    const last = parts.at(-1)
    if (last && last.match === marked[index]) last.text += char
    else parts.push({ text: char, match: marked[index] })
  })
  return parts
}

const kilometers = new Intl.NumberFormat('it-IT', { maximumFractionDigits: 1 })

/** `83` → `80 m`, `1240` → `1,2 km`: as precise as a position on a phone can be. */
export function formatDistance(meters: number): string {
  const rounded = Math.round(meters / 10) * 10
  if (rounded < 1000) return `${rounded} m`
  return `${kilometers.format(rounded / 1000)} km`
}

const dayFormat = new Intl.DateTimeFormat('it-IT', {
  day: '2-digit',
  month: '2-digit',
  timeZone: 'Europe/Rome',
})

/** The calendar day in Turin, `2026-11-02`, to compare two moments. */
function romeDay(date: Date): string {
  return date.toLocaleDateString('en-CA', { timeZone: 'Europe/Rome' })
}

/**
 * "oggi alle 04:00", "ieri alle 04:00", "il 31/10 alle 04:00", in Turin's time. With
 * `short`, today is just the time ("04:00"), for tight rows.
 */
export function formatDayTime(iso: string, now: Date = new Date(), short = false): string {
  const date = new Date(iso)
  const yesterday = new Date(now.getTime() - 24 * 60 * 60 * 1000)
  const time = formatTime(iso)
  if (romeDay(date) === romeDay(now)) return short ? time : `oggi alle ${time}`
  if (romeDay(date) === romeDay(yesterday)) return `ieri alle ${time}`
  return `il ${dayFormat.format(date)} alle ${time}`
}
