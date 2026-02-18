import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  optimizeDeps: {
    include: ['monaco-editor'],
  },
  base: '/_bifrost/',
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    port: 3000,
    proxy: {
      '/_bifrost/api': {
        target: 'http://127.0.0.1:9900',
        changeOrigin: true,
      },
      '/_bifrost/ws': {
        target: 'ws://127.0.0.1:9900',
        ws: true,
      },
    },
  },
});
