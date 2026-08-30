# Extrittio Operations Console

The operations console is a React 18 and TypeScript 5.9 SPA for managing Extrittio
devices, fleets, telemetry, commands, shadows, firmware, rules, alerts, users, and
platform settings.

## Stack

- Vite 7 and React Router 7
- Blueprint.js 6 with the Extrittio-compatible Kantzen UI theme and components
- TanStack Query 5 and Axios for server state
- Zustand 5 for local UI and selection state
- uPlot, Recharts, Leaflet, and force-graph views
- OpenAPI-generated REST types

Feature-owned API, query, component, and page code lives under `src/features`.
Cross-feature UI and infrastructure remain under `src/components`, `src/hooks`,
`src/stores`, and `src/lib`.

## Setup

Node.js 22 and npm 10 are declared in `package.json`. The public
`@patkepa/kantzen-ui` package is installed from the npm registry and does not
require a GitHub Packages token.

```bash
cd apps/frontend
npm ci
npm run dev
```

Vite serves `http://localhost:5173` and proxies `/api` to
`http://localhost:8080`.

## Commands

```bash
npm run build          # TypeScript check and production bundle
npm run lint           # ESLint
npm run lint:fix       # ESLint with safe fixes
npm run format         # Prettier write
npm run format:check   # Prettier verification
npm test               # Dependency-free Node unit tests
npm run generate-api   # Regenerate OpenAPI JSON and TypeScript types
```

REST contract changes must commit both `api/openapi.json` and
`src/types/openapi.ts`. CI regenerates them and rejects drift.

## Authentication and State

The browser uses an HTTP-only session cookie; application code does not persist a
JWT in local storage. TanStack Query owns remote cache state, while Zustand stores
only UI concerns such as shell panels, dialogs, and multi-selection.

Keep new domain code within a feature slice, reuse query keys from
`src/hooks/query-keys.ts`, and use CSS custom properties from
Kantzen UI semantic tokens instead of hard-coded theme colors.
