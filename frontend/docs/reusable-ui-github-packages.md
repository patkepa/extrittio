# Publishing the UI Framework to GitHub Packages

The frontend workspace packages are configured as publishable npm packages for GitHub Packages. The current package scope is `@patkepa`.

GitHub Packages expects the npm scope to match the GitHub user or organization that owns the packages. The current git remote is `github.com/patkepa/extrittio`, so the packages use the `@patkepa` scope.

## Packages

| Package                    | Use in consumer apps                                                |
| -------------------------- | ------------------------------------------------------------------- |
| `@patkepa/app-shell`       | Application chrome, sidebar, top navbar, and error boundary.        |
| `@patkepa/command-palette` | Reusable command palette shell.                                     |
| `@patkepa/data-client`     | Axios client factory.                                               |
| `@patkepa/interactions`    | Keyboard and focus helpers.                                         |
| `@patkepa/navigation`      | Shared navigation and user/project types.                           |
| `@patkepa/theme`           | Theme provider and theme CSS.                                       |
| `@patkepa/ui`              | Shared toolbar, search, status, filter, and empty-state primitives. |

## Local Package Test

Before publishing, create local tarballs and install them in a separate app:

```bash
npm run pack:packages
```

This writes tarballs to:

```text
frontend/dist/package-tarballs/
```

In the consumer app:

```bash
npm install \
  /path/to/extrittio/frontend/dist/package-tarballs/patkepa-theme-0.1.0.tgz \
  /path/to/extrittio/frontend/dist/package-tarballs/patkepa-ui-0.1.0.tgz \
  /path/to/extrittio/frontend/dist/package-tarballs/patkepa-navigation-0.1.0.tgz \
  /path/to/extrittio/frontend/dist/package-tarballs/patkepa-interactions-0.1.0.tgz \
  /path/to/extrittio/frontend/dist/package-tarballs/patkepa-command-palette-0.1.0.tgz \
  /path/to/extrittio/frontend/dist/package-tarballs/patkepa-app-shell-0.1.0.tgz
```

Also install the peer dependencies the app actually uses:

```bash
npm install react react-dom react-router-dom @blueprintjs/core @blueprintjs/icons cmdk axios
```

## Publish to GitHub Packages

Authenticate locally with a GitHub personal access token that can write packages:

```bash
npm login --scope=@patkepa --registry=https://npm.pkg.github.com
```

Then publish all workspace packages:

```bash
npm run publish:packages
```

The package `publishConfig.registry` fields point to `https://npm.pkg.github.com`, and `frontend/.npmrc` routes the `@patkepa` scope to GitHub Packages.

## Use from Another Repository

In the consumer repository, add an `.npmrc` file:

```ini
@patkepa:registry=https://npm.pkg.github.com
```

For private packages, authenticate with a token that has `read:packages`. Locally, this can live in your user-level `~/.npmrc`:

```ini
//npm.pkg.github.com/:_authToken=${GITHUB_PACKAGES_TOKEN}
```

Then install the framework packages:

```bash
npm install \
  @patkepa/theme \
  @patkepa/ui \
  @patkepa/navigation \
  @patkepa/interactions \
  @patkepa/command-palette \
  @patkepa/app-shell
```

Import the styles once in the consumer app entrypoint:

```tsx
import '@blueprintjs/core/lib/css/blueprint.css';
import '@blueprintjs/icons/lib/css/blueprint-icons.css';
import '@patkepa/theme/theme.css';
import '@patkepa/app-shell/styles.css';
```

Use the shell in the consuming app:

```tsx
import { BrowserRouter } from 'react-router-dom';
import { AppShell } from '@patkepa/app-shell';
import { ThemeProvider } from '@patkepa/theme';
import type { NavGroup } from '@patkepa/navigation';

const navGroups: NavGroup[] = [
  {
    label: 'Operations',
    items: [{ label: 'Overview', icon: 'dashboard', href: '/' }],
  },
];

export function App() {
  return (
    <ThemeProvider>
      <BrowserRouter>
        <AppShell
          productName="Internal Tool"
          collapsedProductName="IT"
          navGroups={navGroups}
          sidebarCollapsed={false}
          onToggleSidebar={() => undefined}
        >
          {/* routes */}
        </AppShell>
      </BrowserRouter>
    </ThemeProvider>
  );
}
```

## CI Publishing

For GitHub Actions, the publishing job needs package write permission:

```yaml
permissions:
  contents: read
  packages: write
```

Use `NODE_AUTH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` for packages published from the same repository. For cross-organization or external publishing, use a PAT stored as a repository secret.
