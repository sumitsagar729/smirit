import { motion } from 'motion/react'

const card =
  'absolute inset-0 rounded-[18px] border border-white/10 shadow-[0_24px_48px_-20px_rgb(0_0_0/0.7)] overflow-hidden'

/** Three fanned photo cards: the empty-library illustration. */
export function PhotoStack() {
  return (
    <div className="relative h-[150px] w-[200px]">
      <motion.div
        className={card}
        style={{ background: 'linear-gradient(160deg,#5b7f95,#2d4456)' }}
        initial={{ rotate: 0 }}
        animate={{ rotate: -11, x: -34, y: 6 }}
        transition={{ delay: 0.15, type: 'spring', stiffness: 120, damping: 14 }}
      />
      <motion.div
        className={card}
        style={{ background: 'linear-gradient(160deg,#9b6a8f,#4f3150)' }}
        initial={{ rotate: 0 }}
        animate={{ rotate: 9, x: 34, y: 4 }}
        transition={{ delay: 0.2, type: 'spring', stiffness: 120, damping: 14 }}
      />
      <div className={card} style={{ background: 'linear-gradient(170deg,#f2c46f 0%,#e08a3a 55%,#8a4a2a 100%)' }}>
        <div className="absolute top-6 right-8 size-8 rounded-full bg-[#fff3cf]/90" />
        <svg className="absolute bottom-0 w-full" viewBox="0 0 200 70" preserveAspectRatio="none" aria-hidden="true">
          <path d="M0 70 L0 42 L52 14 L96 44 L132 24 L200 56 L200 70 Z" fill="#5a2f1f" fillOpacity="0.55" />
          <path d="M0 70 L0 56 L70 34 L120 58 L170 42 L200 52 L200 70 Z" fill="#3a1d14" fillOpacity="0.7" />
        </svg>
      </div>
    </div>
  )
}

/** Soft circular badge holding a feature icon, for "coming soon" screens. */
export function IconHalo({ children }: { children: React.ReactNode }) {
  return (
    <div className="relative grid size-[120px] place-items-center">
      <div className="absolute inset-0 rounded-full bg-accent-soft blur-2xl" />
      <div className="relative grid size-[88px] place-items-center rounded-full border border-border bg-elevated text-accent shadow-panel">
        {children}
      </div>
    </div>
  )
}
