import { describe as group, expect, it } from 'vitest'

import type { Chip } from './bindings'
import { chipsToQuery, describe, sameChip } from './search'

group('chipsToQuery', () => {
  it('combines people, places, dates and flags', () => {
    const chips: Chip[] = [
      { type: 'person', id: 1, name: 'Divik', faceId: null },
      { type: 'person', id: 2, name: 'Aarna', faceId: null },
      { type: 'place', id: 9, name: 'Ooty' },
      { type: 'dates', from: '2021-10-01', to: '2021-10-31', label: 'October 2021' },
      { type: 'kind', kind: 'video' },
      { type: 'only' },
      { type: 'onThisDay' },
    ]
    expect(chipsToQuery(chips, '2026-10-20')).toEqual({
      people: [1, 2],
      place: 9,
      from: '2021-10-01',
      to: '2021-10-31',
      kind: 'video',
      only: true,
      onThisDay: '10-20',
    })
  })

  it('maps an age chip to its person and range', () => {
    const q = chipsToQuery([{ type: 'age', person: 4, fromMonths: 0, toMonths: 12, label: "Aarna's first year" }], '2026-01-01')
    expect(q).toEqual({ age: { person: 4, fromMonths: 0, toMonths: 12 } })
  })

  it('is empty without chips', () => {
    expect(chipsToQuery([], '2026-01-01')).toEqual({})
  })
})

group('describe', () => {
  it('reads naturally', () => {
    const chips: Chip[] = [
      { type: 'person', id: 1, name: 'Sumit', faceId: null },
      { type: 'person', id: 2, name: 'Malika', faceId: null },
      { type: 'only' },
      { type: 'trip', id: 3, title: 'Goa' },
    ]
    expect(describe(chips)).toBe('only Sumit and Malika · Goa trip')
  })

  it('compares chips by value', () => {
    expect(sameChip({ type: 'only' }, { type: 'only' })).toBe(true)
    expect(sameChip({ type: 'place', id: 1, name: 'A' }, { type: 'place', id: 2, name: 'A' })).toBe(false)
  })
})
