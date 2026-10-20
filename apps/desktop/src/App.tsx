import { AnimatePresence, motion } from 'motion/react'

import { Sidebar } from './components/Sidebar'
import { Toaster } from './components/Toaster'
import { Viewer } from './components/Viewer'
import { useIndexStatusSync } from './lib/queries'
import { useNav, type View } from './state/nav'
import { ComingSoonView } from './views/ComingSoonView'
import { FoldersView } from './views/FoldersView'
import { PeopleView } from './views/PeopleView'
import { PersonView } from './views/PersonView'
import { SettingsView } from './views/SettingsView'
import { TimelineView } from './views/TimelineView'

function CurrentView({ view }: { view: View }) {
  switch (view) {
    case 'timeline':
      return <TimelineView />
    case 'people':
      return <PeopleView />
    case 'person':
      return <PersonView />
    case 'places':
      return <ComingSoonView feature="places" />
    case 'albums':
      return <ComingSoonView feature="albums" />
    case 'folders':
      return <FoldersView />
    case 'settings':
      return <SettingsView />
  }
}

export function App() {
  const view = useNav((s) => s.view)
  const personId = useNav((s) => s.personId)
  useIndexStatusSync()
  return (
    <div className="flex h-full bg-bg">
      <Sidebar />
      <main className="relative min-w-0 flex-1">
        <AnimatePresence mode="wait" initial={false}>
          <motion.div
            key={view === 'person' ? `person-${personId}` : view}
            className="h-full"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
          >
            <CurrentView view={view} />
          </motion.div>
        </AnimatePresence>
      </main>
      <Viewer />
      <Toaster />
    </div>
  )
}
