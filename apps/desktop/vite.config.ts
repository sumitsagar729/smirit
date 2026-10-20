/// <reference types="vitest/config" />
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { createReadStream, existsSync } from 'node:fs'
import { resolve } from 'node:path'
import { defineConfig, type Plugin } from 'vite'

// In the browser preview (no Tauri), serve the map outlines from the repo's
// data folder at the same names the app's smriti:// protocol uses.
const GEO_FILES: Record<string, string> = {
  countries: 'ne_50m_admin_0_countries.geojson',
  states: 'ne_50m_admin_1_states_provinces.geojson',
}
function devGeo(): Plugin {
  return {
    name: 'smriti-dev-geo',
    configureServer(server) {
      server.middlewares.use('/geo', (req, res, next) => {
        const file = GEO_FILES[(req.url ?? '').replace(/^\//, '')]
        const path = file && resolve(__dirname, '../../data', file)
        if (!path || !existsSync(path)) return next()
        res.setHeader('Content-Type', 'application/geo+json')
        createReadStream(path).pipe(res)
      })
    },
  }
}

// Tauri expects a fixed dev port and must see Rust errors in the terminal.
export default defineConfig({
  plugins: [react(), tailwindcss(), devGeo()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_'],
  build: {
    target: 'es2022',
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    css: false,
  },
})
