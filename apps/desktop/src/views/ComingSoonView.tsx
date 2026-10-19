import { BookImage, MapPinned, Users } from 'lucide-react'

import { IconHalo } from '../components/art'
import { EmptyState } from '../components/ui/EmptyState'

const content = {
  people: {
    icon: <Users size={36} strokeWidth={1.6} />,
    eyebrow: 'Milestone M2',
    title: 'Everyone you love, recognised',
    body: 'Smriti will group faces automatically and learn names in about a minute, including children as they grow from newborn to school age.',
  },
  places: {
    icon: <MapPinned size={36} strokeWidth={1.6} />,
    eyebrow: 'Milestone M3',
    title: 'Every trip on one map',
    body: 'Trips are found from photo locations and folder names, then shown as routes from home with their best photos.',
  },
  albums: {
    icon: <BookImage size={36} strokeWidth={1.6} />,
    eyebrow: 'Version 1.0',
    title: 'Albums that build themselves',
    body: 'Save any search as a smart album, such as "only the four of us" or "every Diwali", and it stays up to date.',
  },
} as const

export function ComingSoonView({ feature }: { feature: keyof typeof content }) {
  const c = content[feature]
  return (
    <div className="grid h-full place-items-center pb-16">
      <EmptyState art={<IconHalo>{c.icon}</IconHalo>} eyebrow={c.eyebrow} title={c.title} body={c.body} />
    </div>
  )
}
