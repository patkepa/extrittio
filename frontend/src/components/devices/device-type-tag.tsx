import { Icon, Tag } from '@blueprintjs/core';
import type { IconName } from '@blueprintjs/core';
import type { CSSProperties } from 'react';
import './device-type-tag.css';

interface DeviceTypeTagProps {
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

export const DeviceTypeTag = ({ name, icon, colorHex }: DeviceTypeTagProps) => {
  const color = isHexColor(colorHex) ? colorHex : DEFAULT_COLOR_HEX;
  const style = {
    '--device-type-color': color,
    '--device-type-bg': alphaHex(color, '24'),
    '--device-type-border': alphaHex(color, '59'),
  } as CSSProperties;

  return (
    <Tag minimal className="device-type-tag" style={style}>
      <Icon icon={(icon || DEFAULT_ICON) as IconName} size={12} />
      <span className="device-type-tag__label">{name}</span>
    </Tag>
  );
};
