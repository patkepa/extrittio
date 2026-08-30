# Kantzen UI Integration

The Extrittio operations console consumes the public
[`@patkepa/kantzen-ui`](https://github.com/patkepa/kantzen-ui) package from npm.
The single package replaces the former local `ui/` workspace and its separate
`@extrittio/*` packages.

## Package surface

- `@patkepa/kantzen-ui` provides workspace components and UI primitives.
- `/theme`, `/navigation`, and `/interactions` provide focused subpath exports.
- `/app-shell` and `/command-palette` provide the optional application chrome.
- `styles.css` is imported once, followed by the app-shell and command-palette
  feature styles.

`src/styles/kantzen-ui-compat.css` is intentionally loaded last. It preserves
Extrittio's established colors, dimensions, and Blueprint-compatible treatment
where Kantzen UI's defaults have evolved. Keep product-specific compatibility
rules there instead of patching framework code or vendoring the package.

## Updating

Update the normal npm dependency and lockfile from `apps/frontend/`, then run:

```bash
npm run format:check
npm run lint
npm test
npm run build
```

Kantzen UI is public; local development and CI do not need `NODE_AUTH_TOKEN` or
an npm scope-specific `.npmrc` file.
