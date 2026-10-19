import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { App } from './App'
import { useToasts } from './state/toast'

function renderApp() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={qc}>
      <App />
    </QueryClientProvider>,
  )
}

describe('App shell', () => {
  it('welcomes a new user with an add-folder action', async () => {
    renderApp()
    expect(await screen.findByText("Your family's story starts here")).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /add a photo folder/i })).toBeInTheDocument()
  })

  it('adds a folder and lists it on the Folders page', async () => {
    vi.spyOn(window, 'prompt').mockReturnValue(String.raw`D:\photos and videos`)
    renderApp()
    fireEvent.click(await screen.findByRole('button', { name: /add a photo folder/i }))
    await screen.findAllByText('Folder added')
    expect(useToasts.getState().toasts.filter((t) => t.title === 'Folder added')).toHaveLength(1)

    fireEvent.click(screen.getByRole('button', { name: 'Folders' }))
    expect(await screen.findByText('photos and videos')).toBeInTheDocument()
  })
})
