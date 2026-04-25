import { defineConfig } from 'vite'
import { devtools } from '@tanstack/devtools-vite'

import { tanstackStart } from '@tanstack/react-start/plugin/vite'

import viteReact from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { nitro } from 'nitro/vite'

const plugins = [
  tailwindcss(),
  tanstackStart(),
  viteReact(),
  nitro({
    rollupConfig: { external: [/^@sentry\//] },
    routeRules: {
      '/assets/**': { headers: { 'Cache-Control': 'public, max-age=31536000, immutable' } },
      '/**': { headers: { 'Cache-Control': 'public, max-age=0, must-revalidate' } },
    },
  }),
]

if (process.env.NODE_ENV !== 'production') {
  plugins.unshift(devtools())
}

const config = defineConfig({
  resolve: { tsconfigPaths: true },
  plugins,
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('@codemirror') || id.includes('@lezer') || id.includes('codemirror')) {
            return 'codemirror'
          }
          if (id.includes('yjs') || id.includes('y-websocket') || id.includes('y-codemirror') || id.includes('lib0')) {
            return 'collab'
          }
        },
      },
    },
  },
})

export default config
