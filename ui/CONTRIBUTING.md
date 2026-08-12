# Contributing

## Development

Use Node.js 22.

```bash
npm ci
npm run format:check
npm run lint
npm run build
npm run build:playground
```

Use `npm run dev:playground` to preview the component playground locally.

## Pull Requests

- Keep changes scoped to the affected package or example.
- Add or update README examples when public APIs change.
- Run the full validation commands before requesting review.
- Do not commit generated `dist/`, package tarballs, local caches, or `node_modules`.

## Package Changes

The public packages live in `packages/*`. The playground in `examples/playground` is private and should not be published.

When preparing a release, bump package versions together unless there is a deliberate reason to release only part of the workspace.
