import type { Chip, Id, Query } from './bindings'

/**
 * Chips -> one query. People chips mean "all of them"; two place or date
 * chips narrow further (the later one wins for single-valued parts).
 */
export function chipsToQuery(chips: Chip[], today: string): Query {
  const q: Query = {}
  const people: Id[] = []
  for (const c of chips) {
    switch (c.type) {
      case 'person':
        people.push(c.id)
        break
      case 'place':
        q.place = c.id
        break
      case 'region':
        q.admin1 = c.admin1
        break
      case 'country':
        q.country = c.country
        break
      case 'trip':
        q.trip = c.id
        break
      case 'dates':
        q.from = c.from
        q.to = c.to
        break
      case 'age':
        q.age = { person: c.person, fromMonths: c.fromMonths, toMonths: c.toMonths }
        break
      case 'onThisDay':
        q.onThisDay = today.slice(5, 10)
        break
      case 'folder':
        q.folder = c.path
        break
      case 'kind':
        q.kind = c.kind
        break
      case 'only':
        q.only = true
        break
    }
  }
  if (people.length) q.people = people
  return q
}

/** Short text for a chip. */
export function chipLabel(c: Chip): string {
  switch (c.type) {
    case 'person':
      return c.name
    case 'place':
      return c.name
    case 'region':
      return c.admin1
    case 'country':
      return c.country
    case 'trip':
      return `${c.title} trip`
    case 'dates':
    case 'age':
      return c.label
    case 'onThisDay':
      return 'On this day'
    case 'folder':
      return c.path
    case 'kind':
      return c.kind === 'video' ? 'Videos' : 'Photos'
    case 'only':
      return 'No one else'
  }
}

/** Same chip (ignoring labels), so it isn't added twice. */
export function sameChip(a: Chip, b: Chip) {
  return JSON.stringify(a) === JSON.stringify(b)
}

/** A sentence for the result count, e.g. "Photos of Divik and Aarna in Ooty". */
export function describe(chips: Chip[]): string {
  const people = chips.filter((c) => c.type === 'person').map(chipLabel)
  const rest = chips.filter((c) => c.type !== 'person' && c.type !== 'only').map(chipLabel)
  const only = chips.some((c) => c.type === 'only')
  const who =
    people.length === 0
      ? ''
      : people.length === 1
        ? people[0]
        : `${people.slice(0, -1).join(', ')} and ${people[people.length - 1]}`
  return [who && (only ? `only ${who}` : who), ...rest].filter(Boolean).join(' · ')
}
