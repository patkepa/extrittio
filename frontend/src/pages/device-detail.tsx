import { useCallback, useEffect, useRef } from 'react';
import { useParams, useSearchParams, Navigate, useNavigate } from 'react-router-dom';
import { Spinner, Callout } from '@blueprintjs/core';
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
import { ErrorBoundary } from '../lib/app-shell';
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
  const toolbarRef = useRef<HTMLDivElement | null>(null);
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
    const toolbarElement = toolbarRef.current;
    if (!toolbarElement) return;

    const tabsContainer = toolbarElement.querySelector<HTMLElement>('.device-toolbar-tabs');
    if (!tabsContainer) return;

    tabsContainer.setAttribute('data-focus-region-initial', 'true');
  }, []);

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
        <div className="device-detail-body">
          <Callout intent="danger" icon="error">
            Failed to load device. It may have been deleted.
          </Callout>
        </div>
      </div>
    );
  }

  if (isLoading || !device) {
    return (
      <div className="device-detail-page">
        <div className="device-detail-body">
          <Spinner />
        </div>
      </div>
    );
  }

  return (
    <div className="device-detail-page" ref={toolbarRef}>
      <DeviceHeader device={device} currentTab={currentTab} onTabChange={handleTabChange} />

      <div className="device-detail-body">
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
    </div>
  );
};
