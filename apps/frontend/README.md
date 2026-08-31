# Extrittio operations console

The operations console is a React 18 and TypeScript 5.9 SPA. It manages devices,
blueprints, fleets, telemetry and analytics, commands and shadows, firmware and
OTA, rules and alerts, users and roles, certificates, API keys, and OpenThread
settings.

## Stack and layout

- Vite 7 and React Router 7
- Blueprint.js 6 plus `@patkepa/kantzen-ui`
- TanStack Query and Axios for server state
- Zustand for local shell and selection state
- uPlot, Leaflet, and force-graph views
- OpenAPI-generated REST types

Feature-owned code lives under `src/features`. Shared components, hooks, stores,
and infrastructure live under `src/components`, `src/hooks`, `src/stores`, and
`src/lib`.

## Setup

Node.js 22 and npm 10 are declared in `package.json`:

```bash
cd apps/frontend
npm ci
npm run dev
```

Vite serves `http://localhost:5173` and proxies `/api` to
`http://localhost:8080`.

## Commands

```bash
npm run dev            # Start Vite with hot module replacement
npm run typecheck      # Type-check without creating a bundle
npm run bundle         # Create a production bundle without type-checking
npm run build          # Type-check and create the production bundle
npm run lint           # Run ESLint
npm run lint:fix       # Apply ESLint fixes
npm run format         # Apply Prettier
npm run format:check   # Check Prettier formatting
npm test               # Run Node-based unit tests
npm run generate-api   # Regenerate OpenAPI JSON and TypeScript types
```

REST contract changes must commit both `api/openapi.json` and
`src/types/openapi.ts`. CI regenerates them and rejects drift.

## State and authentication

The browser uses an HTTP-only session cookie and does not persist a JWT in local
storage. TanStack Query owns remote cache state. Zustand stores only UI concerns
such as shell panels, dialogs, and multi-selection.

Keep new domain behavior inside a feature slice, reuse query keys from
`src/hooks/query-keys.ts`, and keep API types generated rather than manually
duplicated.

## Kantzen UI

The public `@patkepa/kantzen-ui` package supplies components, theme tokens,
navigation, interactions, application shell, and command palette exports. It is
installed from npm and does not need a package token.

Global package styles load before
`src/styles/kantzen-ui-compat.css`. Keep Extrittio-specific compatibility and
product treatment in that file instead of patching or vendoring the package.
When updating the dependency, update `package-lock.json` and run the frontend
format, lint, test, and build commands above.
