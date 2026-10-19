import '@testing-library/jest-dom/vitest'

import { cleanup } from '@testing-library/react'
import { afterEach } from 'vitest'

import { useToasts } from '../state/toast'

afterEach(() => {
  cleanup()
  useToasts.setState({ toasts: [] })
})
