import { useParams, useSearchParams, Navigate } from 'react-router-dom';
import { Spinner, Tabs, Tab, Callout } from '@blueprintjs/core';
import { useDevice } from '../hooks/use-devices';
import { DeviceHeader } from '../components/devices/device-header';
import { OverviewTab } from '../components/devices/overview-tab';
import { TelemetryTab } from '../components/devices/telemetry-tab';
import { LogsTab } from '../components/devices/logs-tab';
import { ConfigTab } from '../components/devices/config-tab';
import { ShadowTab } from '../components/devices/shadow-tab';
import { OtaTab } from '../components/devices/ota-tab';
import './device-detail.css';

const VALID_TABS = ['overview', 'telemetry', 'logs', 'config', 'shadow', 'ota'];

export const DeviceDetail = () => {
  const { deviceId } = useParams<{ deviceId: string }>();
  const [searchParams, setSearchParams] = useSearchParams();
  const { data: device, isLoading, error } = useDevice(deviceId ?? null);

  const activeTab = searchParams.get('tab') ?? 'overview';
  const currentTab = VALID_TABS.includes(activeTab) ? activeTab : 'overview';

  const handleTabChange = (newTab: string) => {
    if (newTab === 'overview') {
      searchParams.delete('tab');
    } else {
      searchParams.set('tab', newTab);
    }
    setSearchParams(searchParams, { replace: true });
  };

  if (!deviceId) return <Navigate to="/devices" replace />;

  if (error) {
    return (
      <div className="device-detail-page">
        <Callout intent="danger" icon="error">
          Failed to load device. It may have been deleted.
        </Callout>
      </div>
    );
  }

  if (isLoading || !device) {
    return (
      <div className="device-detail-page">
        <Spinner />
      </div>
    );
  }

  return (
    <div className="device-detail-page">
      <DeviceHeader device={device} />

      <div className="detail-tabs">
        <Tabs
          id="device-detail-tabs"
          selectedTabId={currentTab}
          onChange={(newTab) => handleTabChange(newTab as string)}
          large
        >
          <Tab id="overview" title="Overview" />
          <Tab id="telemetry" title="Telemetry" />
          <Tab id="logs" title="Logs" />
          <Tab id="config" title="Config" />
          <Tab id="shadow" title="Shadow" />
          <Tab id="ota" title="OTA" />
        </Tabs>
      </div>

      <div className="detail-tab-content">
        {currentTab === 'overview' && <OverviewTab device={device} />}
        {currentTab === 'telemetry' && <TelemetryTab deviceId={device.id} />}
        {currentTab === 'logs' && <LogsTab deviceId={device.id} />}
        {currentTab === 'config' && <ConfigTab deviceId={device.id} />}
        {currentTab === 'shadow' && <ShadowTab deviceId={device.id} />}
        {currentTab === 'ota' && <OtaTab device={device} />}
      </div>
    </div>
  );
};
