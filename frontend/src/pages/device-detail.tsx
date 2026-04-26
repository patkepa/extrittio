import { useCallback, useEffect, useRef } from 'react';
import { useParams, useSearchParams, Navigate, useNavigate } from 'react-router-dom';
import { Spinner, Tabs, Tab, Callout } from '@blueprintjs/core';
import { useDevice } from '../hooks/use-devices';
import { DeviceHeader } from '../components/devices/device-header';
import { OverviewTab } from '../components/devices/overview-tab';
import { TelemetryTab } from '../components/devices/telemetry-tab';
import { LocationTab } from '../components/devices/location-tab';
import { LogsTab } from '../components/devices/logs-tab';
import { ConfigTab } from '../components/devices/config-tab';
import { ShadowTab } from '../components/devices/shadow-tab';
import { OtaTab } from '../components/devices/ota-tab';
import { CommandsTab } from '../components/devices/commands-tab';
import { AlertsTab } from '../components/devices/alerts-tab';
import { ErrorBoundary } from '../components/error-boundary';
import { getDirectionalKey, shouldIgnorePageShortcut } from '../utils/keyboard';
import './device-detail.css';

const VALID_TABS = [
  'overview',
  'shadow',
  'commands',
  'telemetry',
  'location',
  'ota',
  'config',
  'logs',
  'alerts',
];

export const DeviceDetail = () => {
  const { deviceId } = useParams<{ deviceId: string }>();
  const navigate = useNavigate();
  const tabsRef = useRef<HTMLDivElement | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();
  const { data: device, isLoading, error } = useDevice(deviceId ?? null);

  const activeTab = searchParams.get('tab') ?? 'overview';
  const currentTab = VALID_TABS.includes(activeTab) ? activeTab : 'overview';

  const handleTabChange = useCallback(
    (newTab: string) => {
      setSearchParams(
        (prev) => {
          const next = new URLSearchParams(prev);
          if (newTab === 'overview') {
            next.delete('tab');
          } else {
            next.set('tab', newTab);
          }
          return next;
        },
        { replace: true },
      );
    },
    [setSearchParams],
  );

  useEffect(() => {
    const tabsElement = tabsRef.current;
    if (!tabsElement) return;

    tabsElement
      .querySelectorAll<HTMLElement>('[data-device-detail-initial-tab="true"]')
      .forEach((element) => {
        element.removeAttribute('data-device-detail-initial-tab');
        element.removeAttribute('data-focus-region-initial');
      });

    const selectedTab = tabsElement.querySelector<HTMLElement>(
      '[role="tab"][aria-selected="true"]',
    );
    if (!selectedTab) return;

    selectedTab.setAttribute('data-device-detail-initial-tab', 'true');
    selectedTab.setAttribute('data-focus-region-initial', 'true');
  }, [currentTab]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (shouldIgnorePageShortcut(event)) return;

      if (event.key === 'Escape' || event.key.toLowerCase() === 'b') {
        event.preventDefault();
        navigate('/devices');
        return;
      }

      const direction = getDirectionalKey(event);
      if (!direction || direction === 'up' || direction === 'down') return;

      const currentIndex = VALID_TABS.indexOf(currentTab);
      let nextIndex = currentIndex;
      if (direction === 'left') nextIndex = currentIndex - 1;
      if (direction === 'right') nextIndex = currentIndex + 1;
      if (direction === 'first') nextIndex = 0;
      if (direction === 'last') nextIndex = VALID_TABS.length - 1;

      nextIndex = Math.min(Math.max(nextIndex, 0), VALID_TABS.length - 1);
      if (nextIndex === currentIndex) return;

      event.preventDefault();
      handleTabChange(VALID_TABS[nextIndex]!);
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [currentTab, handleTabChange, navigate]);

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

      <div className="detail-tabs" ref={tabsRef}>
        <Tabs
          id="device-detail-tabs"
          selectedTabId={currentTab}
          onChange={(newTab) => handleTabChange(newTab as string)}
          large
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

      <div className="detail-tab-content">
        {currentTab === 'overview' && (
          <ErrorBoundary>
            <OverviewTab device={device} />
          </ErrorBoundary>
        )}
        {currentTab === 'telemetry' && (
          <ErrorBoundary>
            <TelemetryTab deviceId={device.id} deviceTypeName={device.device_type_name} />
          </ErrorBoundary>
        )}
        {currentTab === 'location' && (
          <ErrorBoundary>
            <LocationTab
              deviceId={device.id}
              deviceName={device.name}
              deviceStatus={device.status}
            />
          </ErrorBoundary>
        )}
        {currentTab === 'logs' && (
          <ErrorBoundary>
            <LogsTab deviceId={device.id} />
          </ErrorBoundary>
        )}
        {currentTab === 'config' && (
          <ErrorBoundary>
            <ConfigTab deviceId={device.id} />
          </ErrorBoundary>
        )}
        {currentTab === 'shadow' && (
          <ErrorBoundary>
            <ShadowTab deviceId={device.id} />
          </ErrorBoundary>
        )}
        {currentTab === 'commands' && (
          <ErrorBoundary>
            <CommandsTab deviceId={device.id} />
          </ErrorBoundary>
        )}
        {currentTab === 'ota' && (
          <ErrorBoundary>
            <OtaTab device={device} />
          </ErrorBoundary>
        )}
        {currentTab === 'alerts' && (
          <ErrorBoundary>
            <AlertsTab deviceId={device.id} />
          </ErrorBoundary>
        )}
      </div>
    </div>
  );
};
