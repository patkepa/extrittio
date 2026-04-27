import { Button, Icon } from '@blueprintjs/core';
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

export const FleetGraphBottomToolbar = ({
  deviceCount,
  hasGraphData,
}: FleetGraphBottomToolbarProps) => {
  const selectedCount = useSelectionStore((state) => state.selectedDeviceIds.size);
  const actionTarget =
    selectedCount > 0
      ? `${selectedCount.toLocaleString()} device${selectedCount !== 1 ? 's' : ''} selected`
      : `${deviceCount.toLocaleString()} device${deviceCount !== 1 ? 's' : ''} visible`;

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
          <Button
            className={toolbarWideButtonClass}
            icon="flows"
            text="Assign fleet"
            small
            disabled={!hasGraphData}
            onClick={() => showMockToast('Assign fleet')}
          />
          <Button
            className={toolbarWideButtonClass}
            icon="refresh"
            text="Restart"
            small
            disabled={!hasGraphData}
            onClick={() => showMockToast('Restart')}
          />
          <Button
            className={toolbarWideButtonClass}
            icon="cloud-upload"
            text="Trigger OTA"
            small
            disabled={!hasGraphData}
            onClick={() => showMockToast('Trigger OTA')}
          />
          <Button
            className={toolbarWideButtonClass}
            icon="export"
            text="Export"
            small
            disabled={!hasGraphData}
            onClick={() => showMockToast('Export')}
          />
        </div>
        <div className="fleet-graph-toolbar-spacer" />
      </div>
    </BottomToolbar>
  );
};
