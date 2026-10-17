import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  // Assets are served from a custom protocol, keep every reference relative.
  base: './',
  // Tauri waits on this exact port, so never let Vite silently pick another one.
  server: {
    port: 5173,
    strictPort: true,
  },
})
