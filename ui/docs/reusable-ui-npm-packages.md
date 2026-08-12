# Publishing the UI Framework to npm

The reusable UI framework lives in `github.com/extrittio/ui` and publishes versioned public npm packages under the `@extrittio` scope.

Consumer apps should install these packages through normal npm package imports. During local development before the first npm release, a consumer app can keep a workspace copy of `packages/*`; after the first `@extrittio/*` release is available, remove the local workspace copy and install the same versions from npm.

## Packages

| Package                      | Use in consumer apps                                                      |
| ---------------------------- | ------------------------------------------------------------------------- |
| `@extrittio/app-shell`       | Workspace and public site chrome, navigation, footer, and error boundary. |
| `@extrittio/command-palette` | Reusable command palette shell.                                           |
| `@extrittio/data-client`     | Axios client factory.                                                     |
| `@extrittio/interactions`    | Keyboard and focus helpers.                                               |
| `@extrittio/navigation`      | Shared workspace and site navigation and user/project types.              |
| `@extrittio/theme`           | Theme provider and theme CSS.                                             |
| `@extrittio/ui`              | Workspace controls and public site content primitives.                    |

## Local Package Test

Before publishing, create local tarballs and install them in a separate app:

```bash
npm run pack:packages
```

This writes tarballs to:

```text
dist/package-tarballs/
```

In the consumer app:

```bash
npm install \
  /path/to/ui/dist/package-tarballs/extrittio-theme-0.1.0.tgz \
  /path/to/ui/dist/package-tarballs/extrittio-ui-0.1.0.tgz \
  /path/to/ui/dist/package-tarballs/extrittio-navigation-0.1.0.tgz \
  /path/to/ui/dist/package-tarballs/extrittio-interactions-0.1.0.tgz \
  /path/to/ui/dist/package-tarballs/extrittio-command-palette-0.1.0.tgz \
  /path/to/ui/dist/package-tarballs/extrittio-app-shell-0.1.0.tgz
```

Also install the peer dependencies the app actually uses:

```bash
npm install react react-dom react-router-dom @blueprintjs/core @blueprintjs/icons cmdk axios
```

## Publish to npm

The preferred release path is GitHub Actions in `github.com/extrittio/ui`. The workflow lives at:

```text
.github/workflows/release.yml
```

It runs on:

- tags matching `ui-v*`
- manual `workflow_dispatch` runs from the GitHub Actions UI

Before triggering a release, bump the package versions in `packages/*/package.json`. npm will reject an already-published version.

Release by tag:

```bash
git tag ui-v0.1.1
git push origin ui-v0.1.1
```

The workflow runs `npm ci`, `npm run format:check`, `npm run lint`, `npm run build`, `npm run build:playground`, and then publishes the public package workspaces to npm with `NPM_TOKEN`.

For GitHub Actions publishing, create an npm automation token and save it as a repository secret named `NPM_TOKEN`.

Manual local publishing is still possible when needed:

```bash
npm login --registry=https://registry.npmjs.org
npm run publish:packages
```

The package `publishConfig.registry` fields point to `https://registry.npmjs.org`, and `publishConfig.access` is `public` so scoped packages are published publicly.

## Use from Another Repository

Install the framework packages directly from npm:

```bash
npm install \
  @extrittio/theme \
  @extrittio/ui \
  @extrittio/navigation \
  @extrittio/interactions \
  @extrittio/command-palette \
  @extrittio/app-shell
```

No `.npmrc` registry mapping or package read token is required for consumers.

Import the styles once in the consumer app entrypoint:

```tsx
import "@blueprintjs/core/lib/css/blueprint.css";
import "@blueprintjs/icons/lib/css/blueprint-icons.css";
import "@extrittio/theme/theme.css";
import "@extrittio/app-shell/styles.css";
```

Use the workspace shell in the consuming app:

```tsx
import { BrowserRouter } from "react-router-dom";
import { WorkspaceShell } from "@extrittio/app-shell";
import { ThemeProvider } from "@extrittio/theme";
import type { NavGroup } from "@extrittio/navigation";

const navGroups: NavGroup[] = [
  {
    label: "Operations",
    items: [{ label: "Overview", icon: "dashboard", href: "/" }],
  },
];

export function App() {
  return (
    <ThemeProvider>
      <BrowserRouter>
        <WorkspaceShell
          productName="Internal Tool"
          collapsedProductName="IT"
          navGroups={navGroups}
          sidebarCollapsed={false}
          onToggleSidebar={() => undefined}
        >
          {/* routes */}
        </WorkspaceShell>
      </BrowserRouter>
    </ThemeProvider>
  );
}
```

`AppShell` remains available as a deprecated compatibility alias for `WorkspaceShell`.
