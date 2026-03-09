import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Command } from 'cmdk';
import { Icon } from '@blueprintjs/core';
import type { IconName } from '@blueprintjs/icons';
import { useDevices } from '../../hooks/use-devices';
import { useUIStore } from '../../stores/ui-store';
import './command-palette.css';

interface PageEntry {
  label: string;
  icon: IconName;
  href: string;
}

const pages: PageEntry[] = [
  { label: 'Dashboard', icon: 'dashboard', href: '/' },
  { label: 'Devices', icon: 'mobile-video', href: '/devices' },
  { label: 'Settings', icon: 'cog', href: '/settings' },
];

const MAX_PALETTE_DEVICES = 20;

export const CommandPalette = () => {
  const navigate = useNavigate();
  const { data: devices = [] } = useDevices();
  const open = useUIStore((s) => s.isCommandPaletteOpen);
  const toggleCommandPalette = useUIStore((s) => s.toggleCommandPalette);
  const closeCommandPalette = useUIStore((s) => s.closeCommandPalette);
  const openAddDeviceDialog = useUIStore((s) => s.openAddDeviceDialog);

  // Toggle on Cmd+K / Ctrl+K
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'k' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        toggleCommandPalette();
      }
    };
    document.addEventListener('keydown', handler);
    return () => document.removeEventListener('keydown', handler);
  }, [toggleCommandPalette]);

  const runAction = (cb: () => void) => {
    closeCommandPalette();
    cb();
  };

  const displayedDevices = devices.slice(0, MAX_PALETTE_DEVICES);

  return (
    <Command.Dialog
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) closeCommandPalette();
      }}
      label="Command Palette"
      loop
    >
      {/* Search input */}
      <div className="cmdk-input-wrapper">
        <Icon icon="search" size={16} />
        <Command.Input placeholder="Type a command or search..." />
        <kbd className="cmdk-kbd">ESC</kbd>
      </div>

      <Command.List>
        <Command.Empty>No results found.</Command.Empty>

        {/* Pages */}
        <Command.Group heading="Pages">
          {pages.map((page) => (
            <Command.Item
              key={page.href}
              value={page.label}
              onSelect={() => runAction(() => navigate(page.href))}
            >
              <Icon icon={page.icon} size={16} />
              <span className="cmdk-item-label">{page.label}</span>
            </Command.Item>
          ))}
        </Command.Group>

        {/* Devices */}
        {devices.length > 0 && (
          <Command.Group heading="Devices">
            {displayedDevices.map((device) => (
              <Command.Item
                key={device.id}
                value={device.name}
                keywords={[device.device_type_name, device.id]}
                onSelect={() => runAction(() => navigate(`/devices?device=${device.id}`))}
              >
                <span
                  className={`cmdk-device-led cmdk-device-led--${device.status}`}
                />
                <span className="cmdk-item-label">{device.name}</span>
                <span className="cmdk-item-meta">{device.device_type_name}</span>
              </Command.Item>
            ))}
            {devices.length > MAX_PALETTE_DEVICES && (
              <Command.Item
                value="View all devices"
                onSelect={() => runAction(() => navigate('/devices'))}
              >
                <Icon icon="more" size={16} />
                <span className="cmdk-item-label">
                  View all {devices.length} devices...
                </span>
              </Command.Item>
            )}
          </Command.Group>
        )}

        {/* Actions */}
        <Command.Group heading="Actions">
          <Command.Item
            value="Add new device"
            onSelect={() =>
              runAction(() => {
                openAddDeviceDialog();
                navigate('/devices');
              })
            }
          >
            <Icon icon="add" size={16} />
            <span className="cmdk-item-label">Add new device</span>
          </Command.Item>
        </Command.Group>
      </Command.List>
    </Command.Dialog>
  );
};
