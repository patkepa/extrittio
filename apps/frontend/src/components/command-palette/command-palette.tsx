import { useNavigate } from 'react-router-dom';
import { Command } from 'cmdk';
import { Icon } from '@blueprintjs/core';
import { getCommandPaletteRoutes } from '../../app/routes';
import { hasPermission } from '../../auth/permissions';
import { useDevices } from '../../hooks/use-devices';
import { CommandPaletteShell } from '@patkepa/kantzen-ui/command-palette';
import { StatusLed } from '@patkepa/kantzen-ui';
import { useAuthStore } from '../../stores/auth-store';
import { useUIStore } from '../../stores/ui-store';

const MAX_PALETTE_DEVICES = 20;

export const CommandPalette = () => {
  const navigate = useNavigate();
  const permissions = useAuthStore((s) => s.user?.permissions);
  const pages = getCommandPaletteRoutes(permissions);
  const canReadDevices = hasPermission(permissions, 'devices.read');
  const canManageDevices = hasPermission(permissions, 'devices.manage');
  const devicesQuery = useDevices(
    { limit: MAX_PALETTE_DEVICES, offset: 0 },
    { enabled: canReadDevices },
  );
  const devices = devicesQuery.data?.data ?? [];
  const totalDevices = devicesQuery.data?.total ?? devices.length;
  const open = useUIStore((s) => s.isCommandPaletteOpen);
  const toggleCommandPalette = useUIStore((s) => s.toggleCommandPalette);
  const closeCommandPalette = useUIStore((s) => s.closeCommandPalette);
  const openAddDeviceDialog = useUIStore((s) => s.openAddDeviceDialog);

  const runAction = (cb: () => void) => {
    closeCommandPalette();
    cb();
  };

  const displayedDevices = devices.slice(0, MAX_PALETTE_DEVICES);

  return (
    <CommandPaletteShell
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) closeCommandPalette();
      }}
      onToggle={toggleCommandPalette}
    >
      <Command.Group heading="Pages">
        {pages.map((page) => (
          <Command.Item
            key={page.id}
            value={page.label}
            onSelect={() => runAction(() => navigate(page.href ?? page.path))}
          >
            <Icon icon={page.icon} size={16} />
            <span className="cmdk-item-label">{page.label}</span>
          </Command.Item>
        ))}
      </Command.Group>

      {canReadDevices && devices.length > 0 && (
        <Command.Group heading="Devices">
          {displayedDevices.map((device) => (
            <Command.Item
              key={device.id}
              value={device.name}
              keywords={[device.blueprint_name, device.id]}
              onSelect={() => runAction(() => navigate(`/devices?device=${device.id}`))}
            >
              <StatusLed status={device.status} className="cmdk-status-led" />
              <span className="cmdk-item-label">{device.name}</span>
              <span className="cmdk-item-meta">{device.blueprint_name}</span>
            </Command.Item>
          ))}
          {totalDevices > MAX_PALETTE_DEVICES && (
            <Command.Item
              value="View all devices"
              onSelect={() => runAction(() => navigate('/devices'))}
            >
              <Icon icon="more" size={16} />
              <span className="cmdk-item-label">View all {totalDevices} devices...</span>
            </Command.Item>
          )}
        </Command.Group>
      )}

      {canManageDevices && (
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
      )}
    </CommandPaletteShell>
  );
};
