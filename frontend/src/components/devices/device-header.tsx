import { Button, Tab, Tabs, Tooltip } from '@blueprintjs/core';
import { useNavigate } from 'react-router-dom';
import { MainToolbar, StatusLed } from '@extrittio/ui';
import { showSuccessToast } from '../../utils/toaster';
import type { Device } from '../../types/api';

interface DeviceHeaderProps {
  device: Device;
  currentTab: string;
  onTabChange: (tab: string) => void;
}

export const DeviceHeader = ({ device, currentTab, onTabChange }: DeviceHeaderProps) => {
  const navigate = useNavigate();

  return (
    <MainToolbar className="device-toolbar-shell" ariaLabel="Device detail toolbar">
      <div className="device-toolbar">
        <div className="device-toolbar-actions device-toolbar-actions--left">
          <Button
            icon="arrow-left"
            minimal
            small
            title="Back to devices (B)"
            aria-label="Back to devices"
            onClick={() => navigate('/devices')}
          />
        </div>

        <div className="device-toolbar-divider" aria-hidden="true" />

        <div className="device-toolbar-identity">
          <StatusLed status={device.status} />
          <div className="device-toolbar-title-group">
            <span className="device-toolbar-title">{device.name}</span>
            <Tooltip content="Click to copy" placement="bottom" compact minimal>
              <span
                className="device-toolbar-subtitle mono-data copy-on-click"
                onClick={() =>
                  void navigator.clipboard
                    .writeText(device.id)
                    .then(() => showSuccessToast('Device ID copied'))
                }
              >
                {device.id}
              </span>
            </Tooltip>
          </div>
        </div>

        <div className="device-toolbar-divider" aria-hidden="true" />

        <div className="device-toolbar-tabs" tabIndex={-1}>
          <Tabs
            id="device-detail-tabs"
            selectedTabId={currentTab}
            onChange={(newTab) => onTabChange(newTab as string)}
          >
            <Tab id="overview" title="Overview" />
            <Tab id="shadow" title="Shadow" />
            <Tab id="commands" title="Commands" />
            <Tab id="telemetry" title="Telemetry" />
            <Tab id="location" title="Location" />
            <Tab id="ota" title="OTA" />
            <Tab id="config" title="Config" />
            <Tab id="logs" title="Logs" />
            <Tab id="alerts" title="Alerts" />
          </Tabs>
        </div>
      </div>
    </MainToolbar>
  );
};
