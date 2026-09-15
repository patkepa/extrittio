import { fileURLToPath, URL } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv } from 'vite';

const resolvePath = (path: string) => fileURLToPath(new URL(path, import.meta.url));
const resolveFrontendModule = (path: string) => resolvePath(`./node_modules/${path}`);

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, '.', 'EXTRITTIO_');

  return {
    plugins: [react({ compiler: true })],
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
      rolldownOptions: {
        input: {
          main: resolvePath('./index.html'),
        },
        output: {
          codeSplitting: {
            groups: [
              {
                name: 'charts',
                test: /node_modules[\\/]uplot(?:[\\/]|$)/,
                includeDependenciesRecursively: false,
              },
              {
                name: 'query',
                test: /node_modules[\\/](?:@tanstack[\\/]react-query|axios)(?:[\\/]|$)/,
                includeDependenciesRecursively: false,
              },
              {
                name: 'maps',
                test: /node_modules[\\/](?:@react-leaflet[\\/]core|leaflet|react-leaflet)(?:[\\/]|$)/,
                includeDependenciesRecursively: false,
              },
            ],
          },
        },
      },
    },
    server: {
      proxy: {
        '/api': env.EXTRITTIO_DEV_API_URL ?? 'http://localhost:8080',
      },
    },
  };
});
