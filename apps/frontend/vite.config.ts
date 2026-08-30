import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';

const resolvePath = (path: string) => fileURLToPath(new URL(path, import.meta.url));
const resolveFrontendModule = (path: string) => resolvePath(`./node_modules/${path}`);

export default defineConfig({
  resolve: {
    // Keep a single hooks dispatcher if a locally packed Kantzen UI build is
    // used while developing the framework and application together.
    alias: [
      { find: /^react$/, replacement: resolveFrontendModule('react/index.js') },
      {
        find: /^react\/jsx-runtime$/,
        replacement: resolveFrontendModule('react/jsx-runtime.js'),
      },
      {
        find: /^react\/jsx-dev-runtime$/,
        replacement: resolveFrontendModule('react/jsx-dev-runtime.js'),
      },
      { find: /^react-dom$/, replacement: resolveFrontendModule('react-dom/index.js') },
      {
        find: /^react-dom\/client$/,
        replacement: resolveFrontendModule('react-dom/client.js'),
      },
      {
        find: /^react-dom\/test-utils$/,
        replacement: resolveFrontendModule('react-dom/test-utils.js'),
      },
    ],
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
