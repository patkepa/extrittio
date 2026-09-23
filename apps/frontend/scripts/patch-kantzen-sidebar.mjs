import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// Kantzen UI 0.1.0 treats the active route as permanently expanded. Keep its
// initial expansion, while allowing an explicit user choice to override it.
const packageRoot = fileURLToPath(new URL('../node_modules/@patkepa/kantzen-ui/', import.meta.url));

function patch(relativePath, changes) {
  const path = `${packageRoot}${relativePath}`;
  let source = readFileSync(path, 'utf8');
  for (const [before, after] of changes) {
    if (source.includes(after)) continue;
    if (!source.includes(before)) throw new Error(`Unexpected Kantzen UI source: ${relativePath}`);
    source = source.replace(before, after);
  }
  writeFileSync(path, source);
}

patch('dist/app-shell/workspace-sidebar.js', [
  ['useState(new Set())', 'useState(new Map())'],
  [
    `            const next = new Set(prev);
            if (next.has(href)) {
                next.delete(href);
            }
            else {
                next.add(href);
            }
            return next;`,
    `            const next = new Map(prev);
            next.set(href, !(next.get(href) ?? activeParentHrefs.has(href)));
            return next;`,
  ],
  ['new Set(prev).add(href)', 'new Map(prev).set(href, true)'],
  [
    `                const next = new Set(prev);
                next.delete(href);
                return next;`,
    `                return new Map(prev).set(href, false);`,
  ],
]);

patch('dist/app-shell/workspace-sidebar-navigation.js', [
  [
    'expandedItemHrefs.has(item.href) || activeParentHrefs.has(item.href)',
    'expandedItemHrefs.get(item.href) ?? activeParentHrefs.has(item.href)',
  ],
]);

patch('dist/app-shell/workspace-sidebar-navigation.d.ts', [
  ['expandedItemHrefs: ReadonlySet<string>', 'expandedItemHrefs: ReadonlyMap<string, boolean>'],
]);
