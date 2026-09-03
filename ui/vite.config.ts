import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

// Tauri 期望相对路径；dev 端口需与 tauri.conf.json 的 devUrl 一致
export default defineConfig({
  base: './',
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: 'es2021',
    outDir: 'dist',
    sourcemap: false,
  },
})
