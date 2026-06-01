import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
  base: './',
  plugins: [react()],
  server: {
    port: 3000,
    fs: {
      // Allow serving files from the parent directory
      allow: ['..'],
    },
  },
  build: {
    outDir: '../dist/web',
    emptyOutDir: true,
  },
})