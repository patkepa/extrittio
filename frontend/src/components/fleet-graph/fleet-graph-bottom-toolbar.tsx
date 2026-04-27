import { Button, Icon, Tooltip } from '@blueprintjs/core';
import { useState, type ReactNode } from 'react';
import { BottomToolbar } from '../layout/bottom-toolbar';
import { useSelectionStore } from '../../stores/selection-store';
import { showWarningToast } from '../../utils/toaster';

interface FleetGraphBottomToolbarProps {
  deviceCount: number;
  hasGraphData: boolean;
}

const showMockToast = (label: string) => {
  void showWarningToast(`${label} is a mock action for now`);
};

const toolbarWideButtonClass = 'panel-toolbar-button panel-toolbar-button--wide';

interface ToolbarTooltipProps {
  title: string;
  description: string;
  children: ReactNode;
}

const ToolbarTooltip = ({ title, description, children }: ToolbarTooltipProps) => (
  <Tooltip
    content={
      <div className="panel-toolbar-tooltip">
        <span className="panel-toolbar-tooltip-title">{title}</span>
        <span className="panel-toolbar-tooltip-description">{description}</span>
      </div>
    }
    hoverOpenDelay={150}
    minimal
    modifiers={{ offset: { enabled: true, options: { offset: [0, 10] } } }}
    placement="top"
    popoverClassName="panel-toolbar-tooltip-popover"
  >
    {children}
  </Tooltip>
);

const mockActionConfig = [
  {
    icon: 'flows' as const,
    label: 'Assign fleet',
    description: 'Move the current device set into a fleet.',
  },
  {
    icon: 'refresh' as const,
    label: 'Restart',
    description: 'Queue a restart command for selected devices.',
  },
  {
    icon: 'cloud-upload' as const,
    label: 'Trigger OTA',
    description: 'Start a firmware rollout for this device set.',
  },
  {
    icon: 'export' as const,
    label: 'Export',
    description: 'Export the visible or selected device list.',
  },
];

type DisplayToggleKey = 'labels' | 'alerts' | 'fleets' | 'offline';

const displayToggleConfig: Array<{
  key: DisplayToggleKey;
  icon: 'tag' | 'notifications' | 'flows' | 'offline';
  label: string;
  description: string;
}> = [
  {
    key: 'labels',
    icon: 'tag',
    label: 'Device labels',
    description: 'Show device names directly on the graph.',
  },
  {
    key: 'alerts',
    icon: 'notifications',
    label: 'Alert badges',
    description: 'Show active alert indicators on devices.',
  },
  {
    key: 'fleets',
    icon: 'flows',
    label: 'Fleet groups',
    description: 'Show fleet grouping hints in the topology.',
  },
  {
    key: 'offline',
    icon: 'offline',
    label: 'Offline devices',
    description: 'Include offline devices in the graph view.',
  },
];

export const FleetGraphBottomToolbar = ({
  deviceCount,
  hasGraphData,
}: FleetGraphBottomToolbarProps) => {
  const [displayToggles, setDisplayToggles] = useState<Record<DisplayToggleKey, boolean>>({
    labels: true,
    alerts: true,
    fleets: false,
    offline: true,
  });
  const selectedCount = useSelectionStore((state) => state.selectedDeviceIds.size);
  const actionTarget =
    selectedCount > 0
      ? `${selectedCount.toLocaleString()} device${selectedCount !== 1 ? 's' : ''} selected`
      : `${deviceCount.toLocaleString()} device${deviceCount !== 1 ? 's' : ''} visible`;

  const toggleDisplay = (key: DisplayToggleKey) => {
    setDisplayToggles((current) => ({ ...current, [key]: !current[key] }));
  };

  return (
    <BottomToolbar className="fleet-graph-toolbar-shell" ariaLabel="Fleet graph multi-device toolbar">
      <div className="fleet-graph-toolbar fleet-graph-toolbar--bottom">
        <div className="fleet-graph-toolbar-identity">
          <Icon icon="multi-select" size={16} />
          <div className="fleet-graph-toolbar-title-group">
            <span className="fleet-graph-toolbar-title">Multi-device actions</span>
            <span className="fleet-graph-toolbar-subtitle">{actionTarget}</span>
          </div>
        </div>

        <div className="fleet-graph-toolbar-divider" aria-hidden="true" />

        <div className="fleet-graph-toolbar-actions fleet-graph-toolbar-actions--mock">
          {mockActionConfig.map((action) => (
            <ToolbarTooltip key={action.label} title={action.label} description={action.description}>
              <Button
                className={toolbarWideButtonClass}
                icon={action.icon}
                text={action.label}
                small
                disabled={!hasGraphData}
                aria-label={action.label}
                onClick={() => showMockToast(action.label)}
              />
            </ToolbarTooltip>
          ))}
        </div>

        <div className="fleet-graph-toolbar-divider" aria-hidden="true" />

        <div className="fleet-graph-toolbar-actions" aria-label="Display toggles">
          {displayToggleConfig.map((toggle) => {
            const isActive = displayToggles[toggle.key];
            const verb = isActive ? 'Hide' : 'Show';
            return (
              <ToolbarTooltip
                key={toggle.key}
                title={`${verb} ${toggle.label.toLowerCase()}`}
                description={toggle.description}
              >
                <Button
                  className={`panel-toolbar-switch${isActive ? '' : ' panel-toolbar-switch--off'}`}
                  icon={toggle.icon}
                  minimal
                  small
                  disabled={!hasGraphData}
                  aria-label={`${verb} ${toggle.label.toLowerCase()}`}
                  aria-pressed={isActive}
                  onClick={() => toggleDisplay(toggle.key)}
                />
              </ToolbarTooltip>
            );
          })}
        </div>
        <div className="fleet-graph-toolbar-spacer" />
      </div>
    </BottomToolbar>
  );
};
