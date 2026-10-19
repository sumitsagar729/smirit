import { motion } from 'motion/react'
import type { ReactNode } from 'react'

type Props = {
  art?: ReactNode
  eyebrow?: string
  title: string
  body: ReactNode
  actions?: ReactNode
}

export function EmptyState({ art, eyebrow, title, body, actions }: Props) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 12 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
      className="mx-auto flex max-w-lg flex-col items-center px-6 text-center"
    >
      {art && <div className="mb-8">{art}</div>}
      {eyebrow && (
        <div className="mb-3 text-[11px] font-semibold tracking-[0.14em] text-accent uppercase">{eyebrow}</div>
      )}
      <h2 className="font-display text-[30px] leading-tight font-medium tracking-tight text-text">{title}</h2>
      <p className="mt-3 text-[15px] leading-relaxed text-muted">{body}</p>
      {actions && <div className="mt-8 flex items-center gap-3">{actions}</div>}
    </motion.div>
  )
}
