import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';
import path from 'node:path';

export default defineConfig({
  base: './',
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
      '@shared': path.resolve(__dirname, 'shared'),
    },
  },
  build: {
    outDir: 'dist/renderer',
    emptyOutDir: true,
  },
  server: {
    // devUrl 固定 127.0.0.1（IPv4）；不指定 host 时 vite 绑 localhost 可能落在 ::1，导致 Tauri 探测不到
    host: '127.0.0.1',
    port: 5188,
    strictPort: true,
  },
});
