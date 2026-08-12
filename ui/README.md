# Extrittio UI

Reusable UI components for static and interactive sites.

This repository owns the `@extrittio/*` UI package workspace:

| Package                      | Purpose                                                                                   |
| ---------------------------- | ----------------------------------------------------------------------------------------- |
| `@extrittio/app-shell`       | Workspace and public site chrome: shells, navigation, footer, and error boundary.         |
| `@extrittio/command-palette` | Reusable command palette frame and keyboard handling.                                     |
| `@extrittio/data-client`     | Backend-agnostic Axios client factory with token and unauthorized hooks.                  |
| `@extrittio/interactions`    | Keyboard, focus-region, roving-focus, form-navigation, and shortcut hooks.                |
| `@extrittio/navigation`      | Shared workspace and site navigation, badge, project, and user types.                     |
| `@extrittio/theme`           | Theme provider and dark theme CSS custom properties.                                      |
| `@extrittio/ui`              | Workspace controls plus public site sections, heroes, grids, metrics, and CTA primitives. |

## Development

```bash
npm install
npm run build
npm run lint
npm run format:check
```

The packages build typed ESM output into each `packages/*/dist` directory. Generated `dist` folders are not committed.

## Playground

Use the private Vite playground to preview the workspace shell, public site shell, demo frame, and reusable site primitives while developing the packages:

```bash
npm run dev:playground
```

Production-check the playground with:

```bash
npm run build:playground
```

## Release

Packages publish publicly to npm under the `@extrittio` scope.

```bash
npm run pack:packages
npm run publish:packages
```

The preferred release path is the GitHub Actions workflow in `.github/workflows/release.yml`, triggered by `ui-v*` tags or manual dispatch.

Consumer apps can install the packages directly from npm without registry configuration or tokens.

See `docs/reusable-ui-npm-packages.md` for the full consumer and publishing flow.

## License

MIT. See `LICENSE`.
