import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import path from 'path'

const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: { $lib: path.resolve('./src/lib') },
  },
  // Tauri: не открывать браузер, слушать на нужном хосте
  server: {
    host: host || false,
    port: 5173,
    strictPort: true,
    hmr: host ? { protocol: 'ws', host, port: 5183 } : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  // Tauri работает с ES modules
  build: {
    target: 'esnext',
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  clearScreen: false,
})
