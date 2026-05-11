import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';

const resolvePath = (path: string) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  build: {
    target: 'esnext',
    rollupOptions: {
      input: {
        main: resolvePath('./index.html'),
        routerAdminDemo: resolvePath('./router-admin-demo.html'),
      },
      output: {
        manualChunks: {
          blueprint: ['@blueprintjs/core', '@blueprintjs/icons'],
          charts: ['uplot'],
          query: ['@tanstack/react-query', 'axios'],
          maps: ['leaflet', 'react-leaflet'],
        },
      },
    },
  },
  server: {
    proxy: {
      '/api': 'http://localhost:8080',
    },
  },
});
