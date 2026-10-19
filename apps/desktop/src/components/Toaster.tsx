import { CheckCircle2, Info, X, XCircle } from 'lucide-react'
import { AnimatePresence, motion } from 'motion/react'

import { cn } from '../lib/cn'
import { useToasts } from '../state/toast'

const icons = { success: CheckCircle2, error: XCircle, info: Info }
const tones = { success: 'text-success', error: 'text-danger', info: 'text-accent' }

export function Toaster() {
  const { toasts, dismiss } = useToasts()
  return (
    <div className="pointer-events-none fixed right-5 bottom-5 z-50 flex w-[360px] flex-col gap-2" aria-live="polite">
      <AnimatePresence initial={false}>
        {toasts.map((t) => {
          const Icon = icons[t.tone]
          return (
            <motion.div
              key={t.id}
              layout
              initial={{ opacity: 0, y: 16, scale: 0.96 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, x: 40 }}
              transition={{ type: 'spring', stiffness: 420, damping: 32 }}
              role={t.tone === 'error' ? 'alert' : 'status'}
              className="pointer-events-auto flex gap-3 rounded-xl border border-border bg-elevated p-3.5 shadow-panel"
            >
              <Icon size={18} className={cn('mt-0.5 shrink-0', tones[t.tone])} />
              <div className="min-w-0 flex-1">
                <div className="text-[13.5px] font-medium text-text">{t.title}</div>
                {t.body && <div className="selectable mt-0.5 text-[12.5px] break-words text-muted">{t.body}</div>}
              </div>
              <button
                type="button"
                onClick={() => dismiss(t.id)}
                className="shrink-0 rounded-md p-0.5 text-faint hover:text-text"
                aria-label="Dismiss"
              >
                <X size={15} />
              </button>
            </motion.div>
          )
        })}
      </AnimatePresence>
    </div>
  )
}
