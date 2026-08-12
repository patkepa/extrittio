import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';

const resolvePath = (path: string) => fileURLToPath(new URL(path, import.meta.url));

export default defineConfig({
  resolve: {
    // Local @extrittio packages are linked from ../../ui, which has its own
    // node_modules tree. Ensure their hooks share the app's React dispatcher.
    dedupe: ['react', 'react-dom', 'react/jsx-runtime', 'react/jsx-dev-runtime'],
  },
  build: {
    target: 'esnext',
    rollupOptions: {
      input: {
        main: resolvePath('./index.html'),
      },
      output: {
        manualChunks: {
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
