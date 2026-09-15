import { Icon, Tag } from '@blueprintjs/core';
import type { IconName } from '@blueprintjs/core';
import type { CSSProperties } from 'react';
import './blueprint-tag.css';

interface BlueprintTagProps {
  name: string;
  icon?: string | null;
  colorHex?: string | null;
}

const DEFAULT_ICON = 'cube';
const DEFAULT_COLOR_HEX = '#8ABBFF';

function isHexColor(value: string | null | undefined): value is string {
  return /^#[0-9A-Fa-f]{6}$/.test(value ?? '');
}

function alphaHex(colorHex: string, alpha: string) {
  return `${colorHex}${alpha}`;
}

export const BlueprintTag = ({ name, icon, colorHex }: BlueprintTagProps) => {
  const color = isHexColor(colorHex) ? colorHex : DEFAULT_COLOR_HEX;
  const style = {
    '--blueprint-color': color,
    '--blueprint-bg': alphaHex(color, '24'),
    '--blueprint-border': alphaHex(color, '59'),
  } as CSSProperties;

  return (
    <Tag minimal className="blueprint-tag" style={style}>
      <Icon icon={(icon || DEFAULT_ICON) as IconName} size={12} />
      <span className="blueprint-tag__label">{name}</span>
    </Tag>
  );
};
