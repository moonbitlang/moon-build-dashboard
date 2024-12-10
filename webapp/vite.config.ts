import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  base: '/',
  publicDir: 'public',
  server: {
    proxy: {
      '/mac/latest_data.jsonl.gz': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
      '/windows/latest_data.jsonl.gz': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
      '/linux/latest_data.jsonl.gz': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      }
    }
  },
  build: {
    outDir: 'dist',
    assetsDir: 'assets',
  }
})
