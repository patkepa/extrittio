import { IconSize } from '@blueprintjs/icons/lib/esm/iconTypes';
import { splitPathsBySizeLoader } from '@blueprintjs/icons/lib/esm/paths-loaders/splitPathsBySizeLoader';
import type { IconName } from '@blueprintjs/icons';

// --- Pre-built Path2D cache for device-type icons (16×16 viewBox) ---
const iconPathCache = new Map<string, Path2D[]>();
const pendingIconLoads = new Map<string, Promise<Path2D[]>>();
export const DEFAULT_DEVICE_TYPE_ICON: IconName = 'cube';

export function normalizeIconName(iconName?: string): string {
  return iconName?.trim().toLowerCase().replaceAll('_', '-') || DEFAULT_DEVICE_TYPE_ICON;
}

export async function loadIconPaths(iconName?: string): Promise<Path2D[]> {
  const key = normalizeIconName(iconName);
  const cached = iconPathCache.get(key);
  if (cached) return cached;

  const pending = pendingIconLoads.get(key);
  if (pending) return pending;

  const load = (async () => {
    let svgPaths = await splitPathsBySizeLoader(key as IconName, IconSize.STANDARD);
    if (!svgPaths && key !== DEFAULT_DEVICE_TYPE_ICON) {
      svgPaths = await splitPathsBySizeLoader(DEFAULT_DEVICE_TYPE_ICON, IconSize.STANDARD);
    }
    const paths = (svgPaths ?? []).map((d) => new Path2D(d));
    iconPathCache.set(key, paths);
    return paths;
  })().finally(() => {
    pendingIconLoads.delete(key);
  });

  pendingIconLoads.set(key, load);
  return load;
}

export function getIconPaths(iconName?: string): Path2D[] {
  const key = normalizeIconName(iconName);
  return iconPathCache.get(key) ?? iconPathCache.get(DEFAULT_DEVICE_TYPE_ICON) ?? [];
}
