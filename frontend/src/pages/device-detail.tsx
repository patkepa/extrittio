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
import { CommandsTab } from '../components/devices/commands-tab';
import { ErrorBoundary } from '../components/error-boundary';
import './device-detail.css';

const VALID_TABS = ['overview', 'telemetry', 'logs', 'config', 'shadow', 'commands', 'ota'];

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
          <Tab id="commands" title="Commands" />
          <Tab id="ota" title="OTA" />
        </Tabs>
      </div>

      <div className="detail-tab-content">
        {currentTab === 'overview' && <ErrorBoundary><OverviewTab device={device} /></ErrorBoundary>}
        {currentTab === 'telemetry' && <ErrorBoundary><TelemetryTab deviceId={device.id} /></ErrorBoundary>}
        {currentTab === 'logs' && <ErrorBoundary><LogsTab deviceId={device.id} /></ErrorBoundary>}
        {currentTab === 'config' && <ErrorBoundary><ConfigTab deviceId={device.id} /></ErrorBoundary>}
        {currentTab === 'shadow' && <ErrorBoundary><ShadowTab deviceId={device.id} /></ErrorBoundary>}
        {currentTab === 'commands' && <ErrorBoundary><CommandsTab deviceId={device.id} /></ErrorBoundary>}
        {currentTab === 'ota' && <ErrorBoundary><OtaTab device={device} /></ErrorBoundary>}
      </div>
    </div>
  );
};
