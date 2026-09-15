# Extrittio documentation site

This Astro Starlight application renders the repository's canonical Markdown
documentation from [`../../docs`](../../docs). Do not duplicate documentation
under this app: edit the root documentation and the content loader will pick up
the changes.

## Development

Node.js 22 and npm 10 are required.

```bash
npm ci
npm run dev
```

The development server listens on `http://localhost:4321` by default.

## Verification

```bash
npm run check
npm run build
```
