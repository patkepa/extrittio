# Extrittio Frontend Packages

These workspace packages expose the reusable frontend foundation used by the Extrittio app and the router admin playground.

The package source lives in `packages/*/src`, and each package builds typed ESM output into `packages/*/dist`. The app consumes the package names directly, so future projects can use the same imports without depending on Extrittio device, fleet, telemetry, firmware, alert, or rule code.

## Packages

| Package                    | Purpose                                                                                              |
| -------------------------- | ---------------------------------------------------------------------------------------------------- |
| `@patkepa/app-shell`       | Reusable application chrome: shell, sidebar, top navbar, and error boundary.                         |
| `@patkepa/command-palette` | Reusable command palette frame and keyboard handling.                                                |
| `@patkepa/data-client`     | Backend-agnostic Axios client factory with token and unauthorized hooks.                             |
| `@patkepa/interactions`    | Keyboard, focus-region, roving-focus, form-navigation, and confirm-shortcut helpers.                 |
| `@patkepa/navigation`      | Shared navigation, badge, project, and user types.                                                   |
| `@patkepa/theme`           | Theme provider and dark theme CSS custom properties.                                                 |
| `@patkepa/ui`              | Reusable UI primitives such as toolbars, status LEDs, search fields, filter pills, and empty states. |

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
import { BrowserRouter } from 'react-router-dom';
import { ThemeProvider } from '@patkepa/theme';
import { AppShell } from '@patkepa/app-shell';
import type { NavGroup } from '@patkepa/navigation';

import '@blueprintjs/core/lib/css/blueprint.css';
import '@blueprintjs/icons/lib/css/blueprint-icons.css';
import '@patkepa/theme/theme.css';

const navGroups: NavGroup[] = [
  {
    label: 'Operations',
    items: [{ label: 'Overview', icon: 'dashboard', href: '/' }],
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

## GitHub Packages

The workspace packages are configured for GitHub Packages with `publishConfig.registry` set to `https://npm.pkg.github.com`.

Create local tarballs before publishing:

```bash
npm run pack:packages
```

Publish all workspace packages after authenticating with GitHub Packages:

```bash
npm login --scope=@patkepa --registry=https://npm.pkg.github.com
npm run publish:packages
```

Consumer apps need an `.npmrc` entry for the package scope:

```ini
@patkepa:registry=https://npm.pkg.github.com
```

See `docs/reusable-ui-github-packages.md` for the complete publish and consumer setup.

## Runtime Boundaries

Keep domain-specific code outside these packages. Reusable packages should not import Extrittio API hooks, generated OpenAPI types, device/fleet/rule/alert models, Zustand stores, or product-specific pages.

Project-specific wiring should live in app adapters, such as `src/app/extrittio-shell.tsx`, where navigation data, auth behavior, route badges, breadcrumbs, and command entries are injected into the reusable shell.
