# Extrittio Frontend Packages

These workspace packages expose the reusable frontend foundation used by the Extrittio app and the router admin playground.

The canonical home for these packages is `github.com/extrittio/ui`. This in-repo workspace is the local development bridge that lets the app build against the same `@extrittio/*` imports before or during a registry cutover.

The package source lives in `packages/*/src`, and each package builds typed ESM output into `packages/*/dist`. The app consumes the package names directly, so future projects can use the same imports without depending on Extrittio device, fleet, telemetry, firmware, alert, or rule code.

## Packages

| Package                      | Purpose                                                                                              |
| ---------------------------- | ---------------------------------------------------------------------------------------------------- |
| `@extrittio/app-shell`       | Reusable application chrome: shell, sidebar, top navbar, and error boundary.                         |
| `@extrittio/command-palette` | Reusable command palette frame and keyboard handling.                                                |
| `@extrittio/data-client`     | Backend-agnostic Axios client factory with token and unauthorized hooks.                             |
| `@extrittio/interactions`    | Keyboard, focus-region, roving-focus, form-navigation, and confirm-shortcut helpers.                 |
| `@extrittio/navigation`      | Shared navigation, badge, project, and user types.                                                   |
| `@extrittio/theme`           | Theme provider and dark theme CSS custom properties.                                                 |
| `@extrittio/ui`              | Reusable UI primitives such as toolbars, status LEDs, search fields, filter pills, and empty states. |

Each package directory has a small `README.md` with its exports and a focused usage example.

## Build

```bash
npm run build:packages
```

The root `npm run build` and `npm run dev` scripts run `build:packages` first so the app resolves package imports from fresh generated outputs.
`build:packages` removes stale package `dist/` folders before rebuilding them.

Reusable code should be added to the matching package `src/` directory. The old in-app `src/lib/*` shim layer has been removed; app code and playgrounds should import the workspace packages directly.

## Basic Usage

```tsx
import { BrowserRouter } from "react-router-dom";
import { ThemeProvider } from "@extrittio/theme";
import { AppShell } from "@extrittio/app-shell";
import type { NavGroup } from "@extrittio/navigation";

import "@blueprintjs/core/lib/css/blueprint.css";
import "@blueprintjs/icons/lib/css/blueprint-icons.css";
import "@extrittio/theme/theme.css";

const navGroups: NavGroup[] = [
  {
    label: "Operations",
    items: [{ label: "Overview", icon: "dashboard", href: "/" }],
  },
];

export function RouterAdminApp() {
  return (
    <ThemeProvider>
      <BrowserRouter>
        <AppShell
          productName="Router Admin"
          navGroups={navGroups}
          sidebarCollapsed={false}
          onToggleSidebar={() => undefined}
        >
          {/* app routes */}
        </AppShell>
      </BrowserRouter>
    </ThemeProvider>
  );
}
```

For a complete non-Extrittio example, see `src/playground/router-admin-demo.tsx`.

## Public npm Packages

The workspace packages are configured for the public npm registry with `publishConfig.registry` set to `https://registry.npmjs.org` and `publishConfig.access` set to `public`.

Create local tarballs before publishing:

```bash
npm run pack:packages
```

Publish all workspace packages after authenticating with npm:

```bash
npm login --registry=https://registry.npmjs.org
npm run publish:packages
```

Consumer apps install directly from npm without an `.npmrc` entry or package registry token:

```bash
npm install @extrittio/theme @extrittio/ui @extrittio/app-shell
```

See `docs/reusable-ui-npm-packages.md` for the complete publish and consumer setup.

## Runtime Boundaries

Keep domain-specific code outside these packages. Reusable packages should not import Extrittio API hooks, generated OpenAPI types, device/fleet/rule/alert models, Zustand stores, or product-specific pages.

Project-specific wiring should live in app adapters, such as `src/app/extrittio-shell.tsx`, where navigation data, auth behavior, route badges, breadcrumbs, and command entries are injected into the reusable shell.
