import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

const webPort = Number(process.env.WEB_PORT ?? 3000);

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
    port: webPort,
    proxy: {
      '/_bifrost/api': {
        target: 'http://127.0.0.1:9900',
        changeOrigin: true,
        ws: true,
      },
      '/_bifrost/ws': {
        target: 'ws://127.0.0.1:9900',
        ws: true,
      },
    },
  },
});
